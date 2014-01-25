#[feature(managed_boxes, macro_rules)];
#[crate_id="header"];

extern mod extra;
extern mod syntax;
extern mod rustc;

use std::{os, io};
use syntax::{abi, ast, attr, visit};
use syntax::parse::token;
use rustc::middle::{privacy, const_eval, ty};

macro_rules! warning {
    ($($tt:tt)*) => {
        write!(&mut io::stderr(), $($tt)*)
    }
}

fn main() {
    use extra::getopts::groups;

    let args = std::os::args();
    let opts = ~[groups::optflag("h", "help", "show this help message")];

    let matches = groups::getopts(args.tail(), opts).unwrap();
    if matches.opts_present([~"h", ~"help"]) {
        println!("{}", groups::usage(args[0], opts));
        return;
    }

    for name in matches.free.iter() {
        let (crate, exported, tcx) = get_ast(Path::new(name.as_slice()));
        let crate_id = attr::find_crateid(crate.attrs).expect("missing crate_id");
        let mut output = io::File::create(&Path::new(crate_id.name + ".h"));
        write!(&mut output,
               "\\#ifdef RUSTCRATE_{id}_H
\\#define RUSTCRATE_{id}_H
\\#include<stdint.h>

// auto generated

", id=crate_id.name);
        {
            let mut v = Visitor {
                exported: exported,
                tcx: tcx,
                writer: &mut output
            };
            visit::walk_crate(&mut v, &crate, ());
        }
        output.write_str("#endif");
    }
}

/// Extract the expanded ast of a crate, along with the codemap which
/// connects source code locations to the actual code.
fn get_ast(path: Path) -> (ast::Crate, privacy::ExportedItems, ty::ctxt) {
    use rustc::driver::{driver, session};
    use rustc::metadata::creader::Loader;
    use syntax::diagnostic;

    // cargo culted from rustdoc_ng :(
    let parsesess = syntax::parse::new_parse_sess(None);
    let input = driver::FileInput(path);

    let sessopts = @session::Options {
        binary: ~"headers",
        //maybe_sysroot: Some(@os::self_exe_path().unwrap().dir_path()),
        outputs: ~[session::OutputDylib],
        .. (*session::basic_options()).clone()
    };


    let diagnostic_handler = diagnostic::mk_handler(None);
    let span_diagnostic_handler =
        diagnostic::mk_span_handler(diagnostic_handler, parsesess.cm);

    let sess = driver::build_session_(sessopts, parsesess.cm,
                                      @diagnostic::DefaultEmitter as @diagnostic::Emitter,
                                      span_diagnostic_handler);

    let cfg = driver::build_configuration(sess);

    let crate = driver::phase_1_parse_input(sess, cfg.clone(), &input);

    let loader = &mut Loader::new(sess);
    let (crate, ast_map) = driver::phase_2_configure_and_expand(sess, cfg, loader, crate);
    let driver::CrateAnalysis {
        exported_items, ty_cx, ..
    } = driver::phase_3_run_analysis_passes(sess, &crate, ast_map);

    (crate, exported_items, ty_cx)
}

struct Visitor<'a> {
    tcx: ty::ctxt,
    exported: privacy::ExportedItems,
    writer: &'a mut Writer,
}

impl<'a> Visitor<'a> {
    fn write_as_c_ty(& mut self, ty: &ast::Ty) {
        match ty.node {
            ast::TyNil | ast::TyBot => self.writer.write_str("void"),
            ast::TyUniq(ty) | ast::TyPtr(ast::MutTy { ty, .. }) |
                ast::TyRptr(_, ast::MutTy { ty, .. }) => {
                self.write_as_c_ty(ty);
                self.writer.write_str("*");
            }
            ast::TyPath(_, _, id) => {
                match self.tcx.def_map.borrow().get().get_copy(&id) {
                    ast::DefTy(did) => {
                        fail!("only primitives supported")
                    }
                    ast::DefPrimTy(prim) => {
                        let s = match prim {
                            ast::TyInt(int) => match int {
                                ast::TyI => "intptr_t",
                                ast::TyI8 => "int8_t",
                                ast::TyI16 => "int16_t",
                                ast::TyI32 => "int32_t",
                                ast::TyI64 => "int64_t"
                            },
                            ast::TyUint(uint) => match uint {
                                ast::TyU => "uintptr_t",
                                ast::TyU8 => "uint8_t",
                                ast::TyU16 => "uint16_t",
                                ast::TyU32 => "uint32_t",
                                ast::TyU64 => "uint64_t"
                            },
                            ast::TyFloat(float) => match float {
                                ast::TyF32 => "float",
                                ast::TyF64 => "double"
                            },
                            ast::TyChar => "uint32_t",
                            ast::TyBool => "uint8_t",
                            ast::TyStr => fail!("str not supported")
                        };
                        self.writer.write_str(s);
                    }
                    _ => { fail!("whut") }
                }
            }

            ast::TyBox(..) => fail!("@ boxes not supported"),
            ast::TyVec(..) => fail!("vectors not supported"),
            ast::TyFixedLengthVec(..) => fail!("fixed length vectors not supported"),
            ast::TyClosure(..) => fail!("closures not supported"),
            ast::TyBareFn(..) => fail!("functions not supported"),
            ast::TyTup(..) => fail!("tuples not supported"),
            ast::TyTypeof(..) | ast::TyInfer => fail!("whut"),
        }
    }
}

impl<'a> visit::Visitor<()> for Visitor<'a> {
    fn visit_item(&mut self, item: &ast::Item, _: ()) {
        let is_exported = self.exported.contains(&item.id);
        match item.node {
            ast::ItemMod(..) => { visit::walk_item(self, item, ()); }
            ast::ItemFn(decl, _, abi, ref gen, _)
                if abi.is_c() && is_exported && gen.ty_params.is_empty() => {
                let name = match attr::first_attr_value_str_by_name(item.attrs, "export_name") {
                    // Use provided name
                    Some(name) => name.to_owned(),

                    // Don't mangle
                    _ if attr::contains_name(item.attrs, "no_mangle")
                        => token::ident_to_str(&item.ident).to_owned(),

                    _ => {
                        warning!("Exported C abi function {} with mangled name, not emitting",
                                 token::ident_to_str(&item.ident));
                        return
                    }
                };

                self.write_as_c_ty(decl.output);
                write!(self.writer, " {}(", name);
                for (i, &ast::Arg { ty, .. }) in decl.inputs.iter().enumerate() {
                    if i != 0 { self.writer.write_str(", "); }
                    self.write_as_c_ty(ty)
                }
                if decl.inputs.is_empty() {
                    self.writer.write_str("void");
                }
                self.writer.write_str(");\n");
            }
            ast::ItemStruct(struct_def, ref gen) if gen.ty_params.is_empty() && is_exported => {
                if struct_def.ctor_id.is_some() {
                    warning!("Exported tuple-struct {} is not emitted (unsupported)",
                             token::ident_to_str(&item.ident));
                    return
                }
                write!(self.writer, "struct {} \\{\n", token::ident_to_str(&item.ident));
                for field in struct_def.fields.iter() {
                    self.writer.write_str("    ");
                    self.write_as_c_ty(field.node.ty);
                    match field.node.kind {
                        ast::NamedField(id, _) => {
                            write!(self.writer, " {};\n", token::ident_to_str(&id));
                        }
                        ast::UnnamedField => unreachable!(),
                    }
                }
                self.writer.write_str("};\n");
            }
            ast::ItemEnum(..) if is_exported => {
                warning!("Exported enum {} is not emitted (unsupported)",
                         token::ident_to_str(&item.ident))
            }
            _ => {}
        }
    }
}
