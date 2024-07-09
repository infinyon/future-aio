use proc_macro::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;


#[proc_macro_attribute]
pub fn main_async(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);

    let ret = &input.sig.output;
    let inputs = &input.sig.inputs;
    let name = &input.sig.ident;
    let body = &input.block;
    let attrs = &input.attrs;
    let vis = &input.vis;

    if name != "main" {
        return TokenStream::from(quote_spanned! { name.span() =>
            compile_error!("only the main function can be tagged with #[fluvio_future::main_async]"),
        });
    }

    if input.sig.asyncness.is_none() {
        return TokenStream::from(quote_spanned! { input.span() =>
            compile_error!("the async keyword is missing from the function declaration"),
        });
    }

    let result = quote! {
        #vis fn main() #ret {

            ::fluvio_future::subscriber::init_logger();

            #(#attrs)*
            async fn main(#inputs) #ret {
                #body
            }

            ::fluvio_future::task::run_block_on(async {
                main().await
            })
        }

    };

    result.into()
}

#[proc_macro_attribute]
pub fn test_async(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);

    let ret = &input.sig.output;
    let name = &input.sig.ident;
    let body = &input.block;
    let attrs = &input.attrs;
    let vis = &input.vis;

    if input.sig.asyncness.is_none() {
        return TokenStream::from(quote_spanned! { input.span() =>
            compile_error!("the async keyword is missing from the function declaration"),
        });
    }

    let result = quote! {
        #[::core::prelude::v1::test]
        #(#attrs)*
        #vis fn #name() #ret {
            ::fluvio_future::subscriber::init_logger();

            ::fluvio_future::task::run_block_on(async { #body })
        }
    };

    result.into()
}
