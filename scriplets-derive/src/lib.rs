extern crate proc_macro;

use proc_macro::TokenStream;
use venial::{parse_declaration, Declaration, AttributeValue};
use quote::quote;

#[proc_macro_derive(ComponentPrototype, attributes(prot_category))]
pub fn component_prototype_derive(input: TokenStream) -> TokenStream {
    let declaration = parse_declaration(input.into()).unwrap();
    if let Declaration::Struct(struct_decl) = declaration {
        let struct_name = struct_decl.name;
        let prot_table_category = struct_decl.attributes.iter().find_map(|attr| {
            if attr.get_single_path_segment()? == "prot_category" {
                if let AttributeValue::Group(_, toks) = &attr.value {
                    Some(toks)
                } else {
                    None
                }
            } else {
                None
            }
        }).unwrap();
        quote! {
            impl ComponentPrototype<'_> for #struct_name {
                fn name(&self) -> &str {
                    &self.name
                }

                fn to_component(&self) -> Self {
                    self.clone()
                }

                fn from_pt(prototypes_table: &ComponentPrototypes, name: &str) -> Option<Self> {
                    prototypes_table.#(#prot_table_category)*.get(name).map(Self::to_component)
                }
            }
        }.into()
    } else {
        quote!{}.into()
    }
}
