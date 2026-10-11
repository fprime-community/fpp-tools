use crate::error::{FileError, IncludeError};
use crate::{Parser, parse};
use fpp_ast::*;
use fpp_core::{File, FileReader, SourceFile, Span, Spanned};
use rustc_hash::FxHashMap;
use std::ops::ControlFlow;

/// The kind of member list that an included file is spliced into, which is
/// the entry point its text is parsed with
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IncludeParentKind {
    State,
    StateMachine,
    TlmPacketSet,
    TlmPacket,
    Topology,
    Module,
    Component,
}

/// The state of the transform: the kind of member list each included file is
/// spliced into
type State = FxHashMap<SourceFile, IncludeParentKind>;

/// Resolve include specifiers
pub struct ResolveIncludes<Reader: FileReader> {
    reader: Reader,
}

impl<Reader: FileReader> ResolveIncludes<Reader> {
    pub fn new(reader: Reader) -> ResolveIncludes<Reader> {
        ResolveIncludes { reader }
    }

    /// Resolve the include specifiers of a list of translation units
    pub fn trans_unit_list(&self, a: &mut State, tul: &mut [TransUnit]) -> ControlFlow<()> {
        for tu in tul {
            self.visit_trans_unit(a, tu)?;
        }
        ControlFlow::Continue(())
    }

    fn component_member(
        &self,
        a: &mut State,
        mut member: ComponentMember,
        out: &mut Vec<ComponentMember>,
    ) {
        match &member {
            ComponentMember::SpecInclude(spec_include) => self.resolve_spec_include(
                a,
                IncludeParentKind::Component,
                spec_include,
                |p| p.component_members(),
                Self::component_member,
                out,
            ),
            _ => {
                let _ = member.visit_mut(a, self);
                out.push(member)
            }
        }
    }

    fn module_member(&self, a: &mut State, mut member: ModuleMember, out: &mut Vec<ModuleMember>) {
        match &mut member {
            ModuleMember::SpecInclude(spec_include) => {
                self.resolve_spec_include(
                    a,
                    IncludeParentKind::Module,
                    spec_include,
                    |p| p.module_members(),
                    Self::module_member,
                    out,
                );
            }
            _ => {
                let _ = member.visit_mut(a, self);
                out.push(member)
            }
        }
    }

    /// All include specifiers are resolved, even ones that occur inside or
    /// transitively inside a template definition.
    fn module_template_member(
        &self,
        a: &mut State,
        member: ModuleMember,
        out: &mut Vec<ModuleMember>,
    ) {
        self.module_member(a, member, out)
    }

    fn state_machine_member(
        &self,
        a: &mut State,
        mut member: StateMachineMember,
        out: &mut Vec<StateMachineMember>,
    ) {
        match &mut member {
            StateMachineMember::SpecInclude(spec_include) => {
                self.resolve_spec_include(
                    a,
                    IncludeParentKind::StateMachine,
                    spec_include,
                    |p| p.state_machine_members(),
                    Self::state_machine_member,
                    out,
                );
            }
            _ => {
                let _ = member.visit_mut(a, self);
                out.push(member)
            }
        }
    }

    fn state_member(&self, a: &mut State, mut member: StateMember, out: &mut Vec<StateMember>) {
        match &mut member {
            StateMember::SpecInclude(spec_include) => {
                self.resolve_spec_include(
                    a,
                    IncludeParentKind::State,
                    spec_include,
                    |p| p.state_members(),
                    Self::state_member,
                    out,
                );
            }
            _ => {
                let _ = member.visit_mut(a, self);
                out.push(member)
            }
        }
    }

    fn tlm_packet_member(
        &self,
        a: &mut State,
        mut member: TlmPacketMember,
        out: &mut Vec<TlmPacketMember>,
    ) {
        match &mut member {
            TlmPacketMember::SpecInclude(spec_include) => {
                self.resolve_spec_include(
                    a,
                    IncludeParentKind::TlmPacket,
                    spec_include,
                    |p| p.tlm_packet_members(),
                    Self::tlm_packet_member,
                    out,
                );
            }
            _ => {
                let _ = member.visit_mut(a, self);
                out.push(member)
            }
        }
    }

    fn tlm_packet_set_member(
        &self,
        a: &mut State,
        mut member: TlmPacketSetMember,
        out: &mut Vec<TlmPacketSetMember>,
    ) {
        match &mut member {
            TlmPacketSetMember::SpecInclude(spec_include) => {
                self.resolve_spec_include(
                    a,
                    IncludeParentKind::TlmPacketSet,
                    spec_include,
                    |p| p.tlm_packet_set_members(),
                    Self::tlm_packet_set_member,
                    out,
                );
            }
            _ => {
                let _ = member.visit_mut(a, self);
                out.push(member)
            }
        }
    }

    fn topology_member(
        &self,
        a: &mut State,
        mut member: TopologyMember,
        out: &mut Vec<TopologyMember>,
    ) {
        match &mut member {
            TopologyMember::SpecInclude(spec_include) => {
                self.resolve_spec_include(
                    a,
                    IncludeParentKind::Topology,
                    spec_include,
                    |p| p.topology_members(),
                    Self::topology_member,
                    out,
                );
            }
            _ => {
                let _ = member.visit_mut(a, self);
                out.push(member)
            }
        }
    }

    fn tu_member(&self, a: &mut State, tum: ModuleMember, out: &mut Vec<ModuleMember>) {
        self.module_member(a, tum, out)
    }

    /// Check that including the file at `included_path` from the location
    /// `including_loc` does not close a cycle of includes
    fn check_for_cycle(including_loc: Span, included_path: &str) -> Result<(), IncludeError> {
        fn check_loc(
            including_loc: Span,
            included_file: &File,
            loc_opt: Option<Span>,
            mut visited_paths: Vec<String>,
        ) -> Result<(), IncludeError> {
            match loc_opt {
                Some(loc) => {
                    let path = loc.file().uri();
                    let is_cycle = File::from_string(&path) == *included_file;
                    visited_paths.insert(0, path);
                    if is_cycle {
                        Err(IncludeError::Cycle {
                            loc: including_loc,
                            paths: visited_paths,
                        })
                    } else {
                        check_loc(
                            including_loc,
                            included_file,
                            loc.including_span(),
                            visited_paths,
                        )
                    }
                }
                None => Ok(()),
            }
        }

        if including_loc.file().uri() == "<stdin>" {
            return Ok(());
        }
        check_loc(
            including_loc,
            &File::from_string(included_path),
            Some(including_loc),
            vec![included_path.to_string()],
        )
    }

    /// Parse the file that the include specifier `node` names with `parser`,
    /// and splice its members, transformed by `transformer`, into `out`
    fn resolve_spec_include<T>(
        &self,
        a: &mut State,
        kind: IncludeParentKind,
        node: &SpecInclude,
        parser: fn(&mut Parser) -> Vec<T>,
        transformer: fn(&ResolveIncludes<Reader>, &mut State, T, &mut Vec<T>),
        out: &mut Vec<T>,
    ) {
        let spec = node;
        let including_loc = spec.file.span();
        let path = match self.reader.resolve(including_loc.file(), &spec.file.data) {
            Ok(path) => path,
            Err(_) => {
                fpp_core::Diagnostic::from(FileError::CannotResolvePath {
                    loc: including_loc,
                    name: spec.file.data.clone(),
                })
                .emit();
                return;
            }
        };

        if let Err(err) = Self::check_for_cycle(including_loc, &path) {
            fpp_core::Diagnostic::from(err).emit();
            return;
        }

        // Read the file
        let content = match self.reader.read(&path) {
            Ok(content) => content,
            Err(_) => {
                fpp_core::Diagnostic::from(FileError::CannotOpen {
                    loc: including_loc,
                    name: path,
                })
                .emit();
                return;
            }
        };

        let included_file = SourceFile::new_with_parent(&path, content, including_loc.file());
        a.insert(included_file, kind);

        // Parse the file
        let members = parse(included_file, parser, Some(including_loc));
        for member in members {
            transformer(self, a, member, out);
        }
    }
}

impl<Reader: FileReader> MutVisitor for ResolveIncludes<Reader> {
    type Break = ();
    type State = State;

    fn visit_trans_unit(
        &self,
        a: &mut Self::State,
        node: &mut TransUnit,
    ) -> ControlFlow<Self::Break> {
        let old_members = std::mem::take(&mut node.0);
        for member in old_members.into_iter() {
            self.tu_member(a, member, &mut node.0)
        }

        ControlFlow::Continue(())
    }

    fn visit_def_component(
        &self,
        a: &mut Self::State,
        node: &mut DefComponent,
    ) -> ControlFlow<Self::Break> {
        let old_members = std::mem::take(&mut node.members);
        for member in old_members.into_iter() {
            self.component_member(a, member, &mut node.members)
        }

        ControlFlow::Continue(())
    }

    fn visit_def_module(
        &self,
        a: &mut Self::State,
        node: &mut DefModule,
    ) -> ControlFlow<Self::Break> {
        let old_members = std::mem::take(&mut node.members);
        for member in old_members.into_iter() {
            self.module_member(a, member, &mut node.members)
        }

        ControlFlow::Continue(())
    }

    fn visit_def_module_template(
        &self,
        a: &mut Self::State,
        node: &mut DefModuleTemplate,
    ) -> ControlFlow<Self::Break> {
        let old_members = std::mem::take(&mut node.members);
        for member in old_members.into_iter() {
            self.module_template_member(a, member, &mut node.members)
        }

        ControlFlow::Continue(())
    }

    fn visit_def_state(
        &self,
        a: &mut Self::State,
        node: &mut DefState,
    ) -> ControlFlow<Self::Break> {
        let old_members = std::mem::take(&mut node.members);
        for member in old_members.into_iter() {
            self.state_member(a, member, &mut node.members)
        }

        ControlFlow::Continue(())
    }

    /// An external state machine has no members to resolve
    fn visit_def_state_machine(
        &self,
        a: &mut Self::State,
        node: &mut DefStateMachine,
    ) -> ControlFlow<Self::Break> {
        if let Some(members) = &mut node.members {
            let old_members = std::mem::take(members);
            for member in old_members.into_iter() {
                self.state_machine_member(a, member, members)
            }
        }

        ControlFlow::Continue(())
    }

    fn visit_spec_tlm_packet(
        &self,
        a: &mut Self::State,
        node: &mut SpecTlmPacket,
    ) -> ControlFlow<Self::Break> {
        let old_members = std::mem::take(&mut node.members);
        for member in old_members.into_iter() {
            self.tlm_packet_member(a, member, &mut node.members)
        }

        ControlFlow::Continue(())
    }

    fn visit_spec_tlm_packet_set(
        &self,
        a: &mut Self::State,
        node: &mut SpecTlmPacketSet,
    ) -> ControlFlow<Self::Break> {
        let old_members = std::mem::take(&mut node.members);
        for member in old_members.into_iter() {
            self.tlm_packet_set_member(a, member, &mut node.members)
        }

        ControlFlow::Continue(())
    }

    fn visit_def_topology(
        &self,
        a: &mut Self::State,
        node: &mut DefTopology,
    ) -> ControlFlow<Self::Break> {
        let old_members = std::mem::take(&mut node.members);
        for member in old_members.into_iter() {
            self.topology_member(a, member, &mut node.members)
        }

        ControlFlow::Continue(())
    }
}
