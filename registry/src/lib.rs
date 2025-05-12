use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::{Expr, Ident, ItemFn, Lit, Token, parse::Parser};
use syn::{Meta, parse_macro_input, punctuated::Punctuated};

fn get_crate(name: &str) -> proc_macro2::TokenStream {
    let crate_found = crate_name(name).expect(&format!(
        "{} must be present in Cargo.toml for #[register] macro to work",
        name
    ));
    return match crate_found {
        FoundCrate::Itself => quote!(crate),
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!(::#ident)
        }
    };
}

fn get_event(event_arg: TokenStream) -> String {
    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;

    let args = parser.parse(event_arg).expect("Failed to parse Event Type");
    let meta = &args[0];

    match meta {
        Meta::NameValue(nv) => {
            let name = nv.path.get_ident().unwrap().to_string();
            if name != "event" {
                let err = format!("Incorrect Identifier: {}\nPlease use 'event'", name);
                panic!("{}", err);
            }
            let value = match &nv.value {
                Expr::Lit(s) => {
                    if let Lit::Str(e) = &s.lit {
                        e.value().to_lowercase()
                    } else {
                        panic!("Incorrect Type for the event value, it should be string only.")
                    }
                }
                _ => panic!("Incorrect Type for the event value, it should be string only."),
            };
            value
        }
        _ => {
            panic!(
                "Incorrect Argument(s) passed to the #[register] macro.\n Hint: Correct Format: #[register(event = 'on_message')] etc"
            );
        }
    }
}

#[proc_macro_attribute]
pub fn register(args: TokenStream, item: TokenStream) -> TokenStream {
    let mayuri = get_crate("mayuri");

    let input_fn: ItemFn = parse_macro_input!(item);
    let fn_name = &input_fn.sig.ident;
    let fn_wrapper_name = format_ident!("__mayuri_wrap_{}", fn_name.to_string());
    let fn_registeration_name = format_ident!("__mayuri_register_{}", fn_name.to_string());

    let event = get_event(args);

    let expanded = quote! {
        #input_fn
        fn #fn_wrapper_name(ctx: ::std::sync::Arc<::mayuri::core::context::Context>) -> #mayuri::core::listener::ListenerFuture {
            Box::pin(#fn_name(ctx))
        }

        #[linkme::distributed_slice(#mayuri::listener::LISTENER_FUTURE_INFO_SLICE)]
        static #fn_registeration_name: #mayuri::core::listener::ListenerFutureInfo = #mayuri::core::listener::ListenerFutureInfo {
            listener_future_callback: #fn_wrapper_name,
            belongs_to: #event,
        };

    };

    TokenStream::from(expanded)
}
