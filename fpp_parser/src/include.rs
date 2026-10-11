use crate::error::ParseError;
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

/// The kind of member list each included file is spliced into
type ResolveIncludesState = FxHashMap<SourceFile, IncludeParentKind>;

/// Resolve include specifiers
pub struct ResolveIncludes<Reader: FileReader> {
    reader: Reader,
}

impl<Reader: FileReader> ResolveIncludes<Reader> {
    pub fn new(reader: Reader) -> ResolveIncludes<Reader> {
        ResolveIncludes { reader }
    }

    /// Check that including the file at `included_path` from the location
    /// `including_span` does not close a cycle of includes
    fn check_for_cycle(including_span: Span, included_path: &str) -> Result<(), ParseError> {
        if including_span.file().uri() == "<stdin>" {
            return Ok(());
        }

        let included_file = File::from_string(included_path);
        let mut visited_paths = vec![included_path.to_string()];
        let mut loc_opt = Some(including_span);
        while let Some(loc) = loc_opt {
            let path = loc.file().uri();
            let is_cycle = File::from_string(&path) == included_file;
            visited_paths.insert(0, path);
            if is_cycle {
                return Err(ParseError::IncludeCycle {
                    span: including_span,
                    paths: visited_paths,
                });
            }
            loc_opt = loc.including_span();
        }

        Ok(())
    }

    /// Parse the file that `spec_include` names with `parser`, and splice its
    /// members, transformed by `transformer`, into `out`
    fn resolve_spec_include<T>(
        &self,
        a: &mut ResolveIncludesState,
        kind: IncludeParentKind,
        spec_include: &SpecInclude,
        parser: fn(&mut Parser) -> Vec<T>,
        transformer: fn(&ResolveIncludes<Reader>, &mut ResolveIncludesState, T, &mut Vec<T>),
        out: &mut Vec<T>,
    ) {
        let including_span = spec_include.file.span();
        let path = match self
            .reader
            .resolve(including_span.file(), &spec_include.file.data)
        {
            Ok(path) => path,
            Err(_) => {
                fpp_core::Diagnostic::from(ParseError::CannotResolvePath {
                    span: including_span,
                    name: spec_include.file.data.clone(),
                })
                .emit();
                return;
            }
        };

        if let Err(err) = Self::check_for_cycle(including_span, &path) {
            fpp_core::Diagnostic::from(err).emit();
            return;
        }

        let content = match self.reader.read(&path) {
            Ok(content) => content,
            Err(_) => {
                fpp_core::Diagnostic::from(ParseError::CannotOpen {
                    span: including_span,
                    name: path,
                })
                .emit();
                return;
            }
        };

        let file = SourceFile::new_with_parent(&path, content, including_span.file());
        a.insert(file, kind);

        let members = parse(file, parser, Some(including_span));
        for member in members {
            transformer(self, a, member, out);
        }
    }

    fn component_member(
        &self,
        a: &mut ResolveIncludesState,
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

    fn topology_member(
        &self,
        a: &mut ResolveIncludesState,
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

    fn module_member(
        &self,
        a: &mut ResolveIncludesState,
        mut member: ModuleMember,
        out: &mut Vec<ModuleMember>,
    ) {
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

    fn state_machine_member(
        &self,
        a: &mut ResolveIncludesState,
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

    fn state_member(
        &self,
        a: &mut ResolveIncludesState,
        mut member: StateMember,
        out: &mut Vec<StateMember>,
    ) {
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
        a: &mut ResolveIncludesState,
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
        a: &mut ResolveIncludesState,
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
}

impl<Reader: FileReader> MutVisitor for ResolveIncludes<Reader> {
    type Break = ();
    type State = ResolveIncludesState;

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

    /// All include specifiers are resolved, even ones that occur inside or
    /// transitively inside a template definition.
    fn visit_def_module_template(
        &self,
        a: &mut Self::State,
        node: &mut DefModuleTemplate,
    ) -> ControlFlow<Self::Break> {
        let old_members = std::mem::take(&mut node.members);
        for member in old_members.into_iter() {
            self.module_member(a, member, &mut node.members)
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

    fn visit_trans_unit(
        &self,
        a: &mut Self::State,
        node: &mut TransUnit,
    ) -> ControlFlow<Self::Break> {
        let old_members = std::mem::take(&mut node.0);
        for member in old_members.into_iter() {
            self.module_member(a, member, &mut node.0)
        }

        ControlFlow::Continue(())
    }
}
