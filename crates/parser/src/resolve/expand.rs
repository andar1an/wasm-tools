use crate::ast::*;
use std::mem;

#[derive(Default)]
pub struct Expander<'a> {
    to_append: Vec<ModuleField<'a>>,
    funcs: u32,
    memories: u32,
    tables: u32,
    globals: u32,
}

fn page_size() -> u32 {
    1 << 16
}

impl<'a> Expander<'a> {
    /// Process all elements of `fields`, with the specified expansion method.
    ///
    /// This method will handle the `to_append` field of this `Expander`,
    /// appending items after processing a `ModuleField` as necessary.
    pub fn process(
        &mut self,
        fields: &mut Vec<ModuleField<'a>>,
        mut f: impl FnMut(&mut Self, &mut ModuleField<'a>),
    ) {
        let mut cur = 0;
        while cur < fields.len() {
            f(self, &mut fields[cur]);
            for new in self.to_append.drain(..) {
                fields.insert(cur, new);
                cur += 1;
            }
            cur += 1;
        }
    }

    /// Inverts inline `import` descriptions into actual `import` statements.
    ///
    /// In doing so this also takes care of inline `export` statements, if any,
    /// since we'll be removing the corresponding `ModuleField` item and
    /// replacing it in-place.
    ///
    /// To work right, `Import` field items must be visited first.
    ///
    /// This will replace `item` in-place with an `Import` directive if an
    /// inline import is found, and after this pass has been run no more inline
    /// import directives should be present.
    pub fn deinline_import(&mut self, item: &mut ModuleField<'a>) {
        match item {
            ModuleField::Func(f) => {
                let (module, name) = match f.kind {
                    FuncKind::Import { module, name } => (module, name),
                    _ => return,
                };
                for name in f.exports.names.drain(..) {
                    self.to_append.push(ModuleField::Export(Export {
                        name,
                        kind: ExportKind::Func(Index::Num(self.funcs)),
                    }));
                }
                *item = ModuleField::Import(Import {
                    span: f.span,
                    module,
                    name,
                    id: f.name,
                    kind: ImportKind::Func(f.ty.clone()),
                });
                self.funcs += 1;
            }

            ModuleField::Memory(m) => {
                let (module, name, ty) = match m.kind {
                    MemoryKind::Import { module, name, ty } => (module, name, ty),
                    _ => return,
                };
                for name in m.exports.names.drain(..) {
                    self.to_append.push(ModuleField::Export(Export {
                        name,
                        kind: ExportKind::Memory(Index::Num(self.memories)),
                    }));
                }
                *item = ModuleField::Import(Import {
                    span: m.span,
                    module,
                    name,
                    id: m.name,
                    kind: ImportKind::Memory(ty),
                });
                self.memories += 1;
            }

            ModuleField::Table(t) => {
                let (module, name, ty) = match t.kind {
                    TableKind::Import { module, name, ty } => (module, name, ty),
                    _ => return,
                };
                for name in t.exports.names.drain(..) {
                    self.to_append.push(ModuleField::Export(Export {
                        name,
                        kind: ExportKind::Table(Index::Num(self.tables)),
                    }));
                }
                *item = ModuleField::Import(Import {
                    span: t.span,
                    module,
                    name,
                    id: t.name,
                    kind: ImportKind::Table(ty),
                });
                self.tables += 1;
            }

            ModuleField::Global(g) => {
                let (module, name) = match g.kind {
                    GlobalKind::Import { module, name } => (module, name),
                    _ => return,
                };
                for name in g.exports.names.drain(..) {
                    self.to_append.push(ModuleField::Export(Export {
                        name,
                        kind: ExportKind::Global(Index::Num(self.globals)),
                    }));
                }
                *item = ModuleField::Import(Import {
                    span: g.span,
                    module,
                    name,
                    id: g.name,
                    kind: ImportKind::Global(g.ty),
                });
                self.globals += 1;
            }

            ModuleField::Import(i) => match i.kind {
                ImportKind::Func(_) => self.funcs += 1,
                ImportKind::Memory(_) => self.memories += 1,
                ImportKind::Table(_) => self.tables += 1,
                ImportKind::Global(_) => self.globals += 1,
            },

            _ => {}
        }
    }

    /// Extracts all inline `export` annotations and creates
    /// `ModuleField::Export` items.
    ///
    /// Note that this executes after the `deinline_import` pass to ensure
    /// indices all line up right.
    pub fn deinline_export(&mut self, item: &mut ModuleField<'a>) {
        match item {
            ModuleField::Func(f) => {
                for name in f.exports.names.drain(..) {
                    self.to_append.push(ModuleField::Export(Export {
                        name,
                        kind: ExportKind::Func(Index::Num(self.funcs)),
                    }));
                }
                self.funcs += 1;
            }

            ModuleField::Memory(m) => {
                for name in m.exports.names.drain(..) {
                    self.to_append.push(ModuleField::Export(Export {
                        name,
                        kind: ExportKind::Memory(Index::Num(self.memories)),
                    }));
                }

                // If data is defined inline insert an explicit `data` module
                // field here instead, switching this to a `Normal` memory.
                if let MemoryKind::Inline(data) = &mut m.kind {
                    let len = data.iter().map(|l| l.len()).sum::<usize>() as u32;
                    let pages = (len + page_size() - 1) / page_size();
                    let kind = MemoryKind::Normal(MemoryType {
                        limits: Limits {
                            min: pages,
                            max: Some(pages),
                        },
                        shared: false,
                    });
                    let data = match mem::replace(&mut m.kind, kind) {
                        MemoryKind::Inline(data) => data,
                        _ => unreachable!(),
                    };
                    self.to_append.push(ModuleField::Data(Data {
                        span: m.span,
                        name: None,
                        kind: DataKind::Active {
                            memory: Index::Num(self.memories),
                            offset: Expression {
                                instrs: vec![Instruction::I32Const(0)],
                            },
                        },
                        data,
                    }));
                }

                self.memories += 1;
            }

            ModuleField::Table(t) => {
                for name in t.exports.names.drain(..) {
                    self.to_append.push(ModuleField::Export(Export {
                        name,
                        kind: ExportKind::Table(Index::Num(self.tables)),
                    }));
                }

                // If data is defined inline insert an explicit `data` module
                // field here instead, switching this to a `Normal` memory.
                if let TableKind::Inline { elems, elem } = &mut t.kind {
                    let kind = TableKind::Normal(TableType {
                        limits: Limits {
                            min: elems.len() as u32,
                            max: Some(elems.len() as u32),
                        },
                        elem: *elem,
                    });
                    let elems = match mem::replace(&mut t.kind, kind) {
                        TableKind::Inline { elems, .. } => elems,
                        _ => unreachable!(),
                    };
                    self.to_append.push(ModuleField::Elem(Elem {
                        span: t.span,
                        name: None,
                        kind: ElemKind::Active {
                            table: Index::Num(self.tables),
                            offset: Expression {
                                instrs: vec![Instruction::I32Const(0)],
                            },
                            elems,
                        },
                    }));
                }

                self.tables += 1;
            }

            ModuleField::Global(g) => {
                for name in g.exports.names.drain(..) {
                    self.to_append.push(ModuleField::Export(Export {
                        name,
                        kind: ExportKind::Global(Index::Num(self.globals)),
                    }));
                }
                self.globals += 1;
            }

            _ => {}
        }
    }
}
