/*
 * Copyright (C) 2019-2025 Ruben De Smet, Markus Törnqvist, 2025 projectmoon
 *
 * Modified Whisperfish actor binding code via proc-macro. The license of
 * Whisperfish follows:
 *
 * Whisperfish is free software: you can redistribute it and/or modify it
 * under the terms of the GNU Affero General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or (at
 * your option) any later version.
 * Whisperfish is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Affero
 * General Public License for more details.
 * You should have received a copy of the GNU Affero General Public License
 * along with this program. If not, see
 * <https://www.gnu.org/licenses/>.
 */
extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;

/// Runs the function body inside `::qt_async::with_executor`.
///
/// Use on every method that Qt calls into.
#[proc_macro_attribute]
pub fn with_executor(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);

    let attrs = &input.attrs;
    let vis = &input.vis;
    let sig = &input.sig;
    let body = &input.block;

    TokenStream::from(quote! {
        #(#attrs)*
        #vis #sig {
            ::qt_async::with_executor(move || #body)
        }
    })
}

/// Turns the Rust struct backing a cxx-qt QObject into an active model.
///
/// Usage: `#[active_model(qobject::MyModel)]` on `struct MyModelRust`.
///
/// The QObject must declare, in its bridge:
///
/// ```ignore
/// #[qproperty(bool, ready, READ = get_ready, NOTIFY)]
/// #[qproperty(*mut ActorRegistry, registry, READ = get_registry, WRITE = set_registry)]
/// type MyModel = super::MyModelRust;
///
/// fn get_ready(self: &MyModel) -> bool;
/// fn get_registry(self: &MyModel) -> *mut ActorRegistry;
/// unsafe fn set_registry(self: Pin<&mut MyModel>, registry: *mut ActorRegistry);
/// ```
///
/// and implement `crate::actor::ActorWiringModel`.
#[proc_macro_attribute]
pub fn active_model(attr: TokenStream, input: TokenStream) -> TokenStream {
    let qobject = syn::parse_macro_input!(attr as syn::Path);
    let mut strukt = syn::parse_macro_input!(input as syn::ItemStruct);

    let syn::Fields::Named(fields) = &mut strukt.fields else {
        return TokenStream::from(quote! {
            compile_error!("expected a struct with named fields");
        });
    };

    fields.named.push(syn::parse_quote! {
        _active_model: crate::actor::ActiveModel<#qobject>
    });

    TokenStream::from(quote! {
        #strukt

        impl crate::actor::ActiveModelObject for #qobject {
            fn active_model(&self) -> &crate::actor::ActiveModel<Self> {
                &::cxx_qt::CxxQtType::rust(self)._active_model
            }

            fn active_model_mut(
                self: ::core::pin::Pin<&mut Self>,
            ) -> &mut crate::actor::ActiveModel<Self> {
                &mut ::cxx_qt::CxxQtType::rust_mut(self).get_mut()._active_model
            }

            fn emit_ready_changed(self: ::core::pin::Pin<&mut Self>) {
                self.ready_changed();
            }
        }

        #[allow(dead_code)]
        impl #qobject {
            fn get_ready(&self) -> bool {
                crate::actor::ActiveModelObject::active_model(self).is_ready()
            }

            fn get_registry(&self) -> *mut crate::actor::ActorRegistry {
                crate::actor::ActiveModelObject::active_model(self).registry()
            }

            fn set_registry(
                self: ::core::pin::Pin<&mut Self>,
                registry: *mut crate::actor::ActorRegistry,
            ) {
                ::qt_async::with_executor(move || crate::actor::set_registry(self, registry))
            }

            fn model_ref(&self) -> crate::actor::ModelRef<Self> {
                crate::actor::ActiveModelObject::active_model(self).model_ref()
            }
        }
    })
}
