use heck::ToUpperCamelCase;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse::Parse, parse::ParseStream, parse_macro_input, FnArg, Ident, ItemFn, ReturnType, Type,
};

struct Attr(Option<Ident>);

impl Parse for Attr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(Attr(None));
        }
        let ident: Ident = input.parse()?;
        Ok(Attr(Some(ident)))
    }
}

#[proc_macro_attribute]
pub fn handler(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse function
    let input_fn = parse_macro_input!(item as ItemFn);
    let attr_args = parse_macro_input!(attr as Attr);

    // Determine struct name
    let struct_ident: Ident = if let Some(first) = attr_args.0 {
        first
    } else {
        let fn_name = input_fn.sig.ident.to_string();
        let camel = fn_name.to_upper_camel_case();
        format_ident!("{}Controller", camel)
    };

    // Extract request type from first argument
    let first_arg = input_fn
        .sig
        .inputs
        .first()
        .expect("handler must have one argument");
    let typed_req_ty = match first_arg {
        FnArg::Typed(pt) => (*pt.ty).clone(),
        _ => panic!("expected typed argument"),
    };

    let req_ty = if let Type::Path(tp) = &typed_req_ty {
        let seg = tp.path.segments.last().expect("bad type");
        if seg.ident != "TypedHandlerRequest" {
            panic!("first argument must be TypedHandlerRequest<T>");
        }
        if let syn::PathArguments::AngleBracketed(ab) = &seg.arguments {
            if let Some(syn::GenericArgument::Type(inner)) = ab.args.first() {
                inner.clone()
            } else {
                panic!("missing generic argument");
            }
        } else {
            panic!("expected generic argument");
        }
    } else {
        panic!("expected TypedHandlerRequest type");
    };

    // Extract response type
    let resp_ty: Type = match &input_fn.sig.output {
        ReturnType::Type(_, ty) => (**ty).clone(),
        ReturnType::Default => syn::parse_quote!(()),
    };

    let fn_name = &input_fn.sig.ident;
    let vis = &input_fn.vis;

    let output = quote! {
        #input_fn

        #[derive(Clone, Copy)]
        #vis struct #struct_ident;

        impl brrtrouter::typed::Handler for #struct_ident {
            type Request = #req_ty;
            type Response = #resp_ty;
            fn handle(&self, req: brrtrouter::typed::TypedHandlerRequest<#req_ty>) -> #resp_ty {
                #fn_name(req)
            }
        }
    };

    output.into()
}
