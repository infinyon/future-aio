extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::ItemFn;
use syn::Ident;
use proc_macro2::Span;


#[proc_macro_attribute]
pub fn test_async(args: TokenStream, item: TokenStream) -> TokenStream {

    use syn::AttributeArgs;

    let attribute_args = syn::parse_macro_input!(args as AttributeArgs);
    let input = syn::parse_macro_input!(item as ItemFn);
    let name = &input.sig.ident;
    let sync_name = format!("{}_sync",name);
    let out_fn_iden = Ident::new(&sync_name, Span::call_site());

    let test_attributes = generate::generate_test_attributes(&attribute_args);

    let expression = quote! {

        #[test]
        #test_attributes
        fn #out_fn_iden()  {

            ::fluvio_future::subscriber::init_logger();

            #input

            let ft = async {
                #name().await
            };

            #[cfg(not(target_arch = "wasm32"))]
            if let Err(err) = ::fluvio_future::task::run_block_on(ft) {
                assert!(false,"error: {:?}",err);
            }
            #[cfg(target_arch = "wasm32")]
            ::fluvio_future::task::run_block_on(ft);

        }
    };

    expression.into()

}


#[proc_macro_attribute]
pub fn test(args: TokenStream, item: TokenStream) -> TokenStream {

    use syn::AttributeArgs;

    let attribute_args = syn::parse_macro_input!(args as AttributeArgs);
    let input = syn::parse_macro_input!(item as ItemFn);
    let name = &input.sig.ident;
    let sync_name = format!("{}_sync",name);
    let out_fn_iden = Ident::new(&sync_name, Span::call_site());

    let test_attributes = generate::generate_test_attributes(&attribute_args);

    let expression = quote! {

        #[test]
        #test_attributes
        fn #out_fn_iden()  {

            ::fluvio_future::subscriber::init_logger();

            #input

            let ft = async {
                #name().await;
            };

            
            ::fluvio_future::task::run_block_on(ft);

        }
    };

    expression.into()

}



mod generate {

    use proc_macro2::TokenStream;
    use syn::NestedMeta;
    use quote::quote;

    pub fn generate_test_attributes(attributes: &Vec<NestedMeta>) -> TokenStream {
        
        let args = attributes.iter().map(|meta| {
            quote! {
                #[#meta]
            }
        });

        quote! {

            #(#args)*

        }
    }
}
