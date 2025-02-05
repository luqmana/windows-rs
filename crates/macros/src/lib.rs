mod build_macro;
mod implement;
mod implement_macro;

use build_macro::*;
use gen::*;
use implement_macro::*;
use syn::parse_macro_input;

struct RawString(String);

impl ToTokens for RawString {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.push_str("r#\"");
        tokens.push_str(&self.0);
        tokens.push_str("\"#");
    }
}

/// A macro for generating WinRT modules to a .rs file at build time.
///
/// This macro can be used to import WinRT APIs from any Windows metadata (winmd) file.
/// It is only intended for use from a crate's build.rs script.
///
/// The macro generates a single `build` function which can be used in build scripts
/// to generate the WinRT bindings. After using the `build` macro, call the
/// generated `build` function somewhere in the build.rs script's main function.
///
/// # Usage
/// To use, you must then specify which types you want to use. These
/// follow the same convention as Rust `use` paths. Types know which other types they depend on so
/// `build` will generate any other WinRT types needed for the specified type to work.
///
/// # Example
/// The following `build!` generates all types inside of the `Microsoft::AI::MachineLearning`
/// namespace.
///
/// ```rust,ignore
/// build!(
///     Microsoft::AI::MachineLearning::*
/// );
/// ```
#[proc_macro]
pub fn build(stream: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let build = parse_macro_input!(stream as BuildMacro);
    let tokens = RawString(build.into_tokens_string());
    let target_dir = std::env::var("PATH").expect("No `PATH` env variable set");
    let end = target_dir.find(';').expect("Path not ending in `;`");
    let target_dir = RawString(target_dir[..end].to_string());

    let tokens = quote! {
        {
            // The following must be injected into the token stream because the `OUT_DIR` and `PROFILE`
            // environment variables are only set when the build script run and not when it is being compiled.

            use ::std::io::Write;
            let mut path = ::std::path::PathBuf::from(
                ::std::env::var("OUT_DIR").expect("No `OUT_DIR` env variable set"),
            );

            path.push("windows.rs");
            ::std::fs::write(&path, #tokens).expect("Could not write generated code to windows.rs");

            let mut cmd = ::std::process::Command::new("rustfmt");
            cmd.arg(&path);
            let _ = cmd.output();

            fn copy(source: &::std::path::Path, destination: &mut ::std::path::PathBuf) {
                if let ::std::result::Result::Ok(entries) = ::std::fs::read_dir(source) {
                    for entry in entries.filter_map(|entry| entry.ok()) {
                        if let ::std::result::Result::Ok(entry_type) = entry.file_type() {
                            let path = entry.path();
                            if let ::std::option::Option::Some(last_path_component) = path.file_name() {
                                let _ = ::std::fs::create_dir_all(&destination);
                                destination.push(last_path_component);
                                if entry_type.is_file() {
                                    let _ = ::std::fs::copy(path, &destination);
                                } else if entry_type.is_dir() {
                                    let _ = ::std::fs::create_dir(&destination);
                                    copy(&path, destination);
                                }
                                destination.pop();
                            }
                        }
                    }
                }
            }

            fn copy_to_profile(source: &::std::path::Path, destination: &::std::path::Path, profile: &str) {
                if let ::std::result::Result::Ok(files) = ::std::fs::read_dir(destination) {
                    for file in files.filter_map(|file| file.ok())  {
                        if let ::std::result::Result::Ok(file_type) = file.file_type() {
                            if file_type.is_dir() {
                                let mut path = file.path();
                                if let ::std::option::Option::Some(filename) = path.file_name() {
                                    if filename == profile {
                                        copy(source, &mut path);
                                    } else {
                                        copy_to_profile(source, &path, profile);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let mut source : ::std::path::PathBuf = ::std::env::var("CARGO_MANIFEST_DIR").expect("No `CARGO_MANIFEST_DIR` env variable set").into();
            source.push(".windows");

            if source.exists() {
                println!("cargo:rerun-if-changed={}", source.to_str().expect("`CARGO_MANIFEST_DIR` not a valid path"));

                // The `target_arch` cfg is not set for build scripts so we need to sniff it out from the environment variable.
                source.push(match ::std::env::var("CARGO_CFG_TARGET_ARCH").expect("No `CARGO_CFG_TARGET_ARCH` env variable set").as_str() {
                    "x86_64" => "x64",
                    "x86" => "x86",
                    "arm" => "arm",
                    "aarch64" => "arm64",
                    unexpected => panic!("Unexpected `{}` architecture set by `CARGO_CFG_TARGET_ARCH`", unexpected),
                });

                if source.exists() {
                    println!("cargo:rustc-link-search=native={}", source.to_str().expect("`CARGO_MANIFEST_DIR` not a valid path"));
                }

                let mut destination : ::std::path::PathBuf = #target_dir.into();
                destination.pop();
                destination.pop();

                let profile = ::std::env::var("PROFILE").expect("No `PROFILE` env variable set");
                copy_to_profile(&source, &destination, &profile);

                destination.push(".windows");
                destination.push("winmd");
                source.pop();
                source.push("winmd");
                copy(&source, &mut destination);
            }
        }
    };

    tokens.as_str().parse().unwrap()
}

#[proc_macro]
pub fn generate(stream: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let build = parse_macro_input!(stream as BuildMacro);

    let mut tokens = String::new();
    tokens.push_str("r#\"");
    tokens.push_str(&build.into_tokens_string());
    tokens.push_str("\"#");
    tokens.parse().unwrap()
}

/// Rust structs can use the [`macro@implement`] attribute macro to implement entire WinRT
/// classes or any combination of existing COM and WinRT interfaces.
///
/// If the attribute [`proc_macro::TokenStream`] contains the name of a WinRT class then all
/// of its interfaces are implemented. Otherwise, whatever interfaces are contained within
/// the attribute TokenStream are implemented.
#[proc_macro_attribute]
pub fn implement(
    attribute: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    implement::gen(attribute, input)
}
