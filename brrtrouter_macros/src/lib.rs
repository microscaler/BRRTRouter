use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, punctuated::Punctuated, token::Comma, Ident, ItemFn, Token, Type};

struct FieldSpec {
    name: Ident,
    ty: Type,
}

impl Parse for FieldSpec {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let ty: Type = input.parse()?;
        Ok(FieldSpec { name, ty })
    }
}

struct FieldList {
    fields: Punctuated<FieldSpec, Comma>,
}

impl Parse for FieldList {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        syn::parenthesized!(content in input);
        let fields = content.parse_terminated(FieldSpec::parse, Token![,])?;
        Ok(FieldList { fields })
    }
}

struct HandlerArgs {
    request: FieldList,
    response: FieldList,
}

impl Parse for HandlerArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let request_ident: Ident = input.parse()?;
        if request_ident != "request" {
            return Err(syn::Error::new(request_ident.span(), "expected 'request'"));
        }
        let request: FieldList = input.parse()?;
        input.parse::<Token![,]>()?;
        let response_ident: Ident = input.parse()?;
        if response_ident != "response" {
            return Err(syn::Error::new(response_ident.span(), "expected 'response'"));
        }
        let response: FieldList = input.parse()?;
        Ok(HandlerArgs { request, response })
    }
}

#[proc_macro_attribute]
pub fn handler(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as HandlerArgs);
    let func = parse_macro_input!(item as ItemFn);
    let fn_name = func.sig.ident.clone();

    let req_struct = format_ident!("Request");
    let res_struct = format_ident!("Response");

    let req_fields = args.request.fields.iter().map(|f| {
        let name = &f.name;
        let ty = &f.ty;
        quote! { pub #name: #ty, }
    });

    let res_fields = args.response.fields.iter().map(|f| {
        let name = &f.name;
        let ty = &f.ty;
        quote! { pub #name: #ty, }
    });

    let req_conversions = args.request.fields.iter().map(|f| {
        let name = &f.name;
        quote! { #name: serde_json::from_value(map.remove(stringify!(#name)).unwrap_or_else(|| serde_json::Value::Null)).map_err(|e| anyhow::anyhow!(e))? }
    });

    let expanded = quote! {
        #[derive(Debug, serde::Deserialize, serde::Serialize)]
        pub struct #req_struct {
            #( #req_fields )*
        }

        #[derive(Debug, serde::Serialize)]
        pub struct #res_struct {
            #( #res_fields )*
        }

        impl std::convert::TryFrom<brrtrouter::dispatcher::HandlerRequest> for #req_struct {
            type Error = anyhow::Error;
            fn try_from(req: brrtrouter::dispatcher::HandlerRequest) -> Result<Self, Self::Error> {
                use serde_json::Value;
                let mut map = match req.body {
                    Some(Value::Object(m)) => m,
                    Some(v) => {
                        let mut m = serde_json::Map::new();
                        m.insert("body".to_string(), v);
                        m
                    }
                    None => serde_json::Map::new(),
                };
                Ok(Self { #( #req_conversions, )* })
            }
        }

        #func

        struct GeneratedHandler;
        impl brrtrouter::typed::Handler for GeneratedHandler {
            type Request = #req_struct;
            type Response = #res_struct;
            fn handle(&self, req: brrtrouter::typed::TypedHandlerRequest<Self::Request>) -> Self::Response {
                #fn_name(req)
            }
        }

        #[allow(dead_code)]
        pub unsafe fn register(dispatcher: &mut brrtrouter::dispatcher::Dispatcher) {
            dispatcher.register_typed(stringify!(#fn_name), GeneratedHandler);
        }
    };

    TokenStream::from(expanded)
}
