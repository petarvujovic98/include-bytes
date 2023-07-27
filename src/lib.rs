use std::path::Path;

use swc_core::common::DUMMY_SP;
use swc_core::ecma::ast::{Ident, MemberExpr, MemberProp};
use swc_core::plugin::{plugin_transform, proxies::TransformPluginProgramMetadata};
use swc_core::{
    ecma::{
        ast::{Callee, Expr, Lit, Program},
        transforms::testing::test,
        visit::{as_folder, FoldWith, VisitMut, VisitMutWith},
    },
    plugin::metadata::TransformPluginMetadataContextKind,
};

pub struct TransformVisitor {
    is_include_bytes: bool,
    cwd: Option<String>,
    #[allow(dead_code)]
    filename: Option<String>,
}

impl TransformVisitor {
    pub fn new(filename: Option<String>, cwd: Option<String>) -> Self {
        Self {
            is_include_bytes: false,
            filename,
            cwd,
        }
    }
}

impl VisitMut for TransformVisitor {
    // Implement necessary visit_mut_* methods for actual custom transform.
    // A comprehensive list of possible visitor methods can be found here:
    // https://rustdoc.swc.rs/swc_ecma_visit/trait.VisitMut.html

    fn visit_mut_callee(&mut self, callee: &mut Callee) {
        if let Callee::Expr(expression) = callee {
            if let Expr::Ident(ident) = &mut **expression {
                if &*ident.sym == "includeBytes" {
                    self.is_include_bytes = true;
                }
            }
        }
    }

    fn visit_mut_expr(&mut self, n: &mut Expr) {
        n.visit_mut_children_with(self);

        if !self.is_include_bytes {
            return;
        }

        let Expr::Call(call) = n else {
            return;
        };

        call.callee = Callee::Expr(Box::new(
            MemberExpr {
                span: DUMMY_SP,
                obj: Box::new(Ident::new("env".into(), DUMMY_SP).into()),
                prop: MemberProp::Ident(
                    Ident::new("latin1_string_to_uint8array".into(), DUMMY_SP).into(),
                ),
            }
            .into(),
        ));

        let Some(first) = call.args.first_mut() else {
            panic!("includeBytes(): should have one argument");
        };

        let Expr::Lit(Lit::Str(string)) = &mut *first.expr else {
            panic!("includeBytes(): should only have a string literal as an argument");
        };

        let Some(cwd) = self.cwd.as_ref() else {
            panic!("includeBytes(): current working directory (cwd) is not set");
        };

        let path = Path::new(cwd).join(&*string.value);

        if !path.exists() {
            panic!("includeBytes(): file does not exist");
        }

        let Ok(contents) = std::fs::read_to_string(path) else {
            panic!("includeBytes(): failed to read file");
        };

        *string = contents.into();

        self.is_include_bytes = false;
    }
}

/// An example plugin function with macro support.
/// `plugin_transform` macro interop pointers into deserialized structs, as well
/// as returning ptr back to host.
///
/// It is possible to opt out from macro by writing transform fn manually
/// if plugin need to handle low-level ptr directly via
/// `__transform_plugin_process_impl(
///     ast_ptr: *const u8, ast_ptr_len: i32,
///     unresolved_mark: u32, should_enable_comments_proxy: i32) ->
///     i32 /*  0 for success, fail otherwise.
///             Note this is only for internal pointer interop result,
///             not actual transform result */`
///
/// This requires manual handling of serialization / deserialization from ptrs.
/// Refer swc_plugin_macro to see how does it work internally.
#[plugin_transform]
pub fn process_transform(program: Program, metadata: TransformPluginProgramMetadata) -> Program {
    let filename = metadata.get_context(&TransformPluginMetadataContextKind::Filename);
    let cwd = metadata.get_context(&TransformPluginMetadataContextKind::Cwd);
    println!("filename: {:?}", filename);
    program.fold_with(&mut as_folder(TransformVisitor::new(filename, cwd)))
}

// An example to test plugin transform.
// Recommended strategy to test plugin's transform is verify
// the Visitor's behavior, instead of trying to run `process_transform` with mocks
// unless explicitly required to do so.
test!(
    Default::default(),
    |_| as_folder(TransformVisitor::new(
        None,
        std::env::current_dir()
            .ok()
            .map(|p| p.to_str().map(|s| s.to_string()).unwrap_or_default()),
    )),
    boo,
    // Input codes
    r#"const s = includeBytes(".gitignore");"#,
    // Output codes after transformed with plugin
    r#"const s = env.latin1_string_to_uint8array("/target\n^target/\ntarget\n");"#
);
