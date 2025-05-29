use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, parse::{Parse, ParseStream}, ItemFn, Token, Ident, Type, Result as SynResult, parenthesized, punctuated::Punctuated};

struct FieldDef {
    ident: Ident,
    ty: Type,
}

impl Parse for FieldDef {
    fn parse(input: ParseStream) -> SynResult<Self> {
        let ident: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let ty: Type = input.parse()?;
        Ok(FieldDef { ident, ty })
    }
}

struct HandlerArgs {
    request: Vec<FieldDef>,
    response: Vec<FieldDef>,
}

impl Parse for HandlerArgs {
    fn parse(input: ParseStream) -> SynResult<Self> {
        let mut request = Vec::new();
        let mut response = Vec::new();
        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            let content;
            parenthesized!(content in input);
            let fields = Punctuated::<FieldDef, Token![,]>::parse_terminated(&content)?;
            match ident.to_string().as_str() {
                "request" => request = fields.into_iter().collect(),
                "response" => response = fields.into_iter().collect(),
                other => return Err(syn::Error::new(ident.span(), format!("unexpected section {}", other))),
            }
            if input.peek(Token![,]) { input.parse::<Token![,]>()?; }
        }
        Ok(HandlerArgs { request, response })
    }
}

fn is_option(ty: &Type) -> bool {
    if let Type::Path(p) = ty {
        if let Some(seg) = p.path.segments.first() {
            return seg.ident == "Option";
        }
    }
    false
}

#[proc_macro_attribute]
pub fn handler(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as HandlerArgs);
    let input = parse_macro_input!(item as ItemFn);
    let fn_vis = &input.vis;
    let sig = &input.sig;
    let block = &input.block;

    // Generate fields
    let req_fields = args.request.iter().map(|f| {
        let ident = &f.ident;
        let ty = &f.ty;
        if is_option(ty) {
            quote! { #[serde(skip_serializing_if = "Option::is_none")] pub #ident: #ty, }
        } else {
            quote! { pub #ident: #ty, }
        }
    });
    let res_fields = args.response.iter().map(|f| {
        let ident = &f.ident;
        let ty = &f.ty;
        if is_option(ty) {
            quote! { #[serde(skip_serializing_if = "Option::is_none")] pub #ident: #ty, }
        } else {
            quote! { pub #ident: #ty, }
        }
    });

    let expanded = quote! {
        #[derive(Debug, ::serde::Deserialize, ::serde::Serialize)]
        pub struct Request {
            #(#req_fields)*
        }

        #[derive(Debug, ::serde::Serialize)]
        pub struct Response {
            #(#res_fields)*
        }

        impl ::std::convert::TryFrom<crate::brrtrouter::dispatcher::HandlerRequest> for Request {
            type Error = anyhow::Error;
            fn try_from(req: crate::brrtrouter::dispatcher::HandlerRequest) -> Result<Self, Self::Error> {
                use serde_json::{Map, Value};
                fn convert(value: &str) -> Value {
                    if let Ok(v) = value.parse::<i64>() { Value::from(v) }
                    else if let Ok(v) = value.parse::<f64>() { Value::from(v) }
                    else if let Ok(v) = value.parse::<bool>() { Value::from(v) }
                    else { Value::String(value.to_string()) }
                }
                let mut data_map = Map::new();
                for (k, v) in req.path_params.iter() { data_map.insert(k.clone(), convert(v)); }
                for (k, v) in req.query_params.iter() { data_map.insert(k.clone(), convert(v)); }
                for (k, v) in req.headers.iter() { data_map.insert(k.clone(), convert(v)); }
                for (k, v) in req.cookies.iter() { data_map.insert(k.clone(), convert(v)); }
                if let Some(body) = req.body {
                    match body {
                        Value::Object(map) => { for (k, v) in map { data_map.insert(k, v); } }
                        other => { data_map.insert("body".to_string(), other); }
                    }
                }
                Ok(serde_json::from_value(Value::Object(data_map))?)
            }
        }

        #fn_vis #sig #block
    };
    TokenStream::from(expanded)
}

