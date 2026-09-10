use crate::cursor::Cursor;
use crate::error::{ParseError, ParseResult};
use crate::token::Token;
use fpp_ast::*;
use fpp_core::{BytePos, SourceFile, Spanned};
use fpp_lexer::KeywordKind::*;
use fpp_lexer::TokenKind::*;
use fpp_lexer::{KeywordKind, TokenKind};

pub struct Parser<'a> {
    cursor: Cursor<'a>,
    include_span: Option<fpp_core::Span>,
}

enum ElementParsingResult<T> {
    Terminated(T),
    Unterminated(T),
    Err(ParseError),
    None,
}

pub fn parse<T>(
    source_file: SourceFile,
    entry: fn(&mut Parser) -> T,
    include_span: Option<fpp_core::Span>,
) -> T {
    // We need our own copy of the source text since it needs to be long-lived
    let content = source_file.read().as_ref().to_string();
    let mut parser = Parser {
        cursor: Cursor::new(source_file, content.as_ref(), include_span),
        include_span,
    };

    let out = entry(&mut parser);
    parser.cursor.emit_errors();
    out
}

impl<'a> Parser<'a> {
    fn peek_span(&mut self, n: usize) -> Option<fpp_core::Span> {
        self.cursor.peek_span(n)
    }

    fn peek(&mut self, n: usize) -> TokenKind {
        self.cursor.peek(n)
    }

    fn next(&mut self) -> Option<Token> {
        self.cursor.next()
    }

    fn consume(&mut self, kind: TokenKind) -> ParseResult<Token> {
        self.cursor.consume(kind)
    }

    #[inline]
    fn node(&self, first_token: fpp_core::Span) -> fpp_core::Node {
        let last_token_span = self.cursor.last_token_span();

        fpp_core::Node::new(fpp_core::Span::new(
            first_token.file(),
            first_token.start().pos(),
            last_token_span.end().pos() - first_token.start().pos(),
            self.include_span,
        ))
    }

    fn name(&mut self) -> ParseResult<Name> {
        let ident = self.consume(Identifier)?;
        Ok(Name {
            node_id: self.node(ident.span()),
            data: ident.text().to_string(),
        })
    }

    fn ident(&mut self) -> ParseResult<Ident> {
        let ident = self.consume(Identifier)?;
        Ok(Ident {
            node_id: self.node(ident.span()),
            data: ident.text().to_string(),
        })
    }

    fn dictionary_opt(&mut self, next: KeywordKind) -> ParseResult<(Token, bool)> {
        if self.peek(0) == Keyword(Dictionary) {
            let first = self.consume_keyword(Dictionary)?;
            self.consume_keyword(next)?;
            Ok((first, true))
        } else {
            Ok((self.consume_keyword(next)?, false))
        }
    }

    fn alias_type(&mut self) -> ParseResult<DefAliasType> {
        let (first, is_dictionary_def) = self.dictionary_opt(Type)?;

        let name = self.name()?;
        self.consume(Equals)?;
        let type_name = self.type_name()?;

        Ok(DefAliasType {
            node_id: self.node(first.span()),
            name,
            type_name,
            is_dictionary_def,
        })
    }

    fn abs_type(&mut self) -> ParseResult<DefAbsType> {
        let first = self.consume_keyword(Type)?;
        let name = self.name()?;

        Ok(DefAbsType {
            node_id: self.node(first.span()),
            name,
        })
    }

    fn def_action(&mut self) -> ParseResult<DefAction> {
        let first = self.consume_keyword(Action)?;
        let name = self.name()?;
        let type_name = match self.peek(0) {
            Colon => {
                self.next();
                Some(self.type_name()?)
            }
            _ => None,
        };

        Ok(DefAction {
            node_id: self.node(first.span()),
            name,
            type_name,
        })
    }

    fn def_array(&mut self) -> ParseResult<DefArray> {
        let (first, is_dictionary_def) = self.dictionary_opt(Array)?;

        let name = self.name()?;

        self.consume(Equals)?;

        let size = self.index()?;
        let elt_type = self.type_name()?;

        let default = match self.peek(0) {
            Keyword(Default) => {
                self.next();
                Some(self.expr()?)
            }
            _ => None,
        };

        let format = match self.peek(0) {
            Keyword(Format) => {
                self.next();
                Some(self.lit_string()?)
            }
            _ => None,
        };

        Ok(DefArray {
            node_id: self.node(first.span()),
            name,
            size,
            elt_type,
            default,
            format,
            is_dictionary_def,
        })
    }

    fn def_choice(&mut self) -> ParseResult<DefChoice> {
        let first = self.consume_keyword(Choice)?;
        let name = self.name()?;

        self.consume(LeftCurly)?;

        self.consume_keyword(If)?;
        let guard = self.ident()?;
        let if_transition = self.transition_expr()?;

        self.consume_keyword(Else)?;
        let else_transition = self.transition_expr()?;

        self.consume(RightCurly)?;

        Ok(DefChoice {
            node_id: self.node(first.span()),
            name,
            guard,
            if_transition,
            else_transition,
        })
    }

    fn def_component(&mut self) -> ParseResult<DefComponent> {
        let (kind, first) = self.component_kind()?;
        self.consume_keyword(Component)?;
        let name = self.name()?;

        self.consume(LeftCurly)?;
        let members = self.component_members();
        self.consume(RightCurly)?;

        Ok(DefComponent {
            node_id: self.node(first.span()),
            kind,
            name,
            members,
        })
    }

    pub fn trans_unit(&mut self) -> TransUnit {
        TransUnit(self.module_members())
    }

    pub fn component_members(&mut self) -> Vec<ComponentMember> {
        self.annotated_element_sequence(&Parser::component_member, Semi, RightCurly)
    }

    fn component_kind(&mut self) -> ParseResult<(ComponentKind, Token)> {
        let ck = match self.peek(0) {
            Keyword(Active) => Ok(ComponentKind::Active),
            Keyword(Passive) => Ok(ComponentKind::Passive),
            Keyword(Queued) => Ok(ComponentKind::Queued),
            _ => Err(self.cursor.err_expected_one_of(
                "component kind expected",
                vec![Keyword(Active), Keyword(Passive), Keyword(Queued)],
            )),
        }?;

        Ok((ck, self.next().unwrap()))
    }

    fn component_member(&mut self) -> ParseResult<ComponentMember> {
        match self.peek(0) {
            Keyword(Dictionary) => match self.peek(1) {
                Keyword(Type) => match self.peek(3) {
                    Equals => Ok(ComponentMember::DefAliasType(self.alias_type()?)),
                    _ => Ok(ComponentMember::DefAbsType(self.abs_type()?)),
                },
                Keyword(Array) => Ok(ComponentMember::DefArray(self.def_array()?)),
                Keyword(Constant) => Ok(ComponentMember::DefConstant(self.def_constant()?)),
                Keyword(Enum) => Ok(ComponentMember::DefEnum(self.def_enum()?)),
                Keyword(Struct) => Ok(ComponentMember::DefStruct(self.def_struct()?)),
                _ => Err(self.cursor.err("dictionary definition expected")),
            },

            Keyword(Type) => match self.peek(2) {
                Equals => Ok(ComponentMember::DefAliasType(self.alias_type()?)),
                _ => Ok(ComponentMember::DefAbsType(self.abs_type()?)),
            },
            Keyword(Array) => Ok(ComponentMember::DefArray(self.def_array()?)),
            Keyword(Constant) => Ok(ComponentMember::DefConstant(self.def_constant()?)),
            Keyword(Enum) => Ok(ComponentMember::DefEnum(self.def_enum()?)),
            Keyword(State) => {
                if self.peek(2) == Keyword(Instance) {
                    Ok(ComponentMember::SpecStateMachineInstance(
                        self.spec_state_machine_instance()?,
                    ))
                } else {
                    Ok(ComponentMember::DefStateMachine(self.def_state_machine()?))
                }
            }
            Keyword(Struct) => Ok(ComponentMember::DefStruct(self.def_struct()?)),
            Keyword(Async | Guarded | Sync)
                if self.peek(1) == Keyword(Command)
                    && !matches!(self.peek(2), Keyword(Recv | Reg | Resp)) =>
            {
                Ok(ComponentMember::SpecCommand(self.spec_command()?))
            }
            Keyword(Async | Guarded | Sync | Output | Command | Text | Time) => Ok(
                ComponentMember::SpecPortInstance(self.spec_port_instance()?),
            ),
            Keyword(Product) => {
                if self.peek(1) == Keyword(Container) {
                    Ok(ComponentMember::SpecContainer(self.spec_container()?))
                } else if self.peek(1) == Keyword(Record) {
                    Ok(ComponentMember::SpecRecord(self.spec_record()?))
                } else {
                    // Special port kind
                    Ok(ComponentMember::SpecPortInstance(
                        self.spec_port_instance()?,
                    ))
                }
            }
            Keyword(Event) => {
                if self.peek(1) == Keyword(Port) {
                    Ok(ComponentMember::SpecPortInstance(
                        self.spec_port_instance()?,
                    ))
                } else {
                    Ok(ComponentMember::SpecEvent(self.spec_event()?))
                }
            }
            Keyword(Include) => Ok(ComponentMember::SpecInclude(self.spec_include()?)),
            Keyword(Internal) => Ok(ComponentMember::SpecInternalPort(
                self.spec_internal_port()?,
            )),
            Keyword(Match) => Ok(ComponentMember::SpecPortMatching(
                self.spec_port_matching()?,
            )),
            Keyword(Param) if self.peek(1) == Keyword(Get) || self.peek(1) == Keyword(Set) => Ok(
                ComponentMember::SpecPortInstance(self.spec_port_instance()?),
            ),
            Keyword(Param | External) => Ok(ComponentMember::SpecParam(self.spec_param()?)),
            Keyword(Telemetry) if self.peek(1) == Keyword(Port) => Ok(
                ComponentMember::SpecPortInstance(self.spec_port_instance()?),
            ),
            Keyword(Telemetry) => Ok(ComponentMember::SpecTlmChannel(self.spec_tlm_channel()?)),
            Keyword(Import) => Ok(ComponentMember::SpecInterfaceImport(
                self.spec_import_interface()?,
            )),
            _ => Err(self.cursor.err_expected_one_of(
                "component member expected",
                vec![
                    Keyword(Type),
                    Keyword(Array),
                    Keyword(Constant),
                    Keyword(Enum),
                    Keyword(State),
                    Keyword(Struct),
                    Keyword(Async),
                    Keyword(Sync),
                    Keyword(Guarded),
                    Keyword(Command),
                    Keyword(Text),
                    Keyword(Time),
                    Keyword(Product),
                    Keyword(Event),
                    Keyword(Include),
                    Keyword(Internal),
                    Keyword(Match),
                    Keyword(External),
                    Keyword(Param),
                    Keyword(Telemetry),
                    Keyword(Import),
                ],
            )),
        }
    }

    fn def_module(&mut self) -> ParseResult<DefModule> {
        let first = self.consume_keyword(Module)?;
        let name = self.name()?;
        self.consume(LeftCurly)?;
        let members = self.module_members();
        self.consume(RightCurly)?;

        Ok(DefModule {
            node_id: self.node(first.span()),
            name,
            members,
        })
    }

    pub fn module_members(&mut self) -> Vec<ModuleMember> {
        self.annotated_element_sequence(&Parser::module_member, Semi, RightCurly)
    }

    fn module_member(&mut self) -> ParseResult<ModuleMember> {
        match self.peek(0) {
            Keyword(Dictionary) => match self.peek(1) {
                Keyword(Type) => match self.peek(3) {
                    Equals => Ok(ModuleMember::DefAliasType(self.alias_type()?)),
                    _ => Ok(ModuleMember::DefAbsType(self.abs_type()?)),
                },
                Keyword(Array) => Ok(ModuleMember::DefArray(self.def_array()?)),
                Keyword(Constant) => Ok(ModuleMember::DefConstant(self.def_constant()?)),
                Keyword(Enum) => Ok(ModuleMember::DefEnum(self.def_enum()?)),
                Keyword(Struct) => Ok(ModuleMember::DefStruct(self.def_struct()?)),
                _ => Err(self.cursor.err("dictionary definition expected")),
            },

            Keyword(Type) => match self.peek(2) {
                Equals => Ok(ModuleMember::DefAliasType(self.alias_type()?)),
                _ => Ok(ModuleMember::DefAbsType(self.abs_type()?)),
            },
            Keyword(Array) => Ok(ModuleMember::DefArray(self.def_array()?)),
            Keyword(Constant) => Ok(ModuleMember::DefConstant(self.def_constant()?)),
            Keyword(Enum) => Ok(ModuleMember::DefEnum(self.def_enum()?)),
            Keyword(Struct) => Ok(ModuleMember::DefStruct(self.def_struct()?)),
            Keyword(Instance) => Ok(ModuleMember::DefComponentInstance(
                self.def_component_instance()?,
            )),
            Keyword(Passive) | Keyword(Active) | Keyword(Queued) => {
                Ok(ModuleMember::DefComponent(self.def_component()?))
            }
            Keyword(Interface) => Ok(ModuleMember::DefInterface(self.def_interface()?)),
            Keyword(Module) => Ok(ModuleMember::DefModule(self.def_module()?)),
            Keyword(Port) => Ok(ModuleMember::DefPort(self.def_port()?)),
            Keyword(State) => Ok(ModuleMember::DefStateMachine(self.def_state_machine()?)),
            Keyword(System) => Ok(ModuleMember::DefSystem(self.def_system()?)),
            Keyword(Deployment) | Keyword(Topology) => {
                Ok(ModuleMember::DefTopology(self.def_topology()?))
            }
            Keyword(Include) => Ok(ModuleMember::SpecInclude(self.spec_include()?)),
            Keyword(Locate) => Ok(ModuleMember::SpecLoc(self.spec_loc()?)),
            _ => Err(self.cursor.err_expected_one_of(
                "module member expected",
                vec![
                    Keyword(Type),
                    Keyword(Array),
                    Keyword(Constant),
                    Keyword(Enum),
                    Keyword(Struct),
                    Keyword(Component),
                    Keyword(Active),
                    Keyword(Passive),
                    Keyword(Queued),
                    Keyword(Interface),
                    Keyword(Module),
                    Keyword(Port),
                    Keyword(State),
                    Keyword(System),
                    Keyword(Deployment),
                    Keyword(Topology),
                    Keyword(Include),
                    Keyword(Locate),
                ],
            )),
        }
    }

    fn def_port(&mut self) -> ParseResult<DefPort> {
        let first = self.consume_keyword(Port)?;
        let name = self.name()?;
        let params = self.formal_param_list()?;
        let return_type = match self.peek(0) {
            RightArrow => {
                self.next();
                Some(self.type_name()?)
            }
            _ => None,
        };

        Ok(DefPort {
            node_id: self.node(first.span()),
            name,
            params,
            return_type,
        })
    }

    fn spec_loc(&mut self) -> ParseResult<SpecLoc> {
        let first = self.consume_keyword(Locate)?;

        let is_dictionary_def = {
            if self.peek(0) == Keyword(Dictionary) {
                self.consume_keyword(Dictionary)?;
                true
            } else {
                false
            }
        };

        let kind = match self.peek(0) {
            Keyword(Component) => {
                self.next();
                SpecLocKind::Component
            }
            Keyword(Constant) => {
                self.next();
                SpecLocKind::Constant
            }
            Keyword(Instance) => {
                self.next();
                SpecLocKind::Instance
            }
            Keyword(Port) => {
                self.next();
                SpecLocKind::Port
            }
            Keyword(State) => {
                self.next();
                self.consume_keyword(Machine)?;
                SpecLocKind::StateMachine
            }
            Keyword(System) => {
                self.next();
                SpecLocKind::System
            }
            Keyword(Type) => {
                self.next();
                SpecLocKind::Type
            }
            Keyword(Interface) => {
                self.next();
                SpecLocKind::Interface
            }
            _ => {
                return Err(self.cursor.err_expected_one_of(
                    "location kind expected",
                    vec![
                        Keyword(Component),
                        Keyword(Constant),
                        Keyword(Instance),
                        Keyword(Port),
                        Keyword(State),
                        Keyword(System),
                        Keyword(Type),
                        Keyword(Interface),
                    ],
                ));
            }
        };

        let symbol = self.qual_ident()?;
        self.consume_keyword(At)?;
        let file = self.lit_string()?;
        Ok(SpecLoc {
            node_id: self.node(first.span()),
            kind,
            symbol,
            file,
            is_dictionary_def,
        })
    }

    fn def_topology(&mut self) -> ParseResult<DefTopology> {
        let (is_deployment, first) = match self.peek(0) {
            Keyword(Deployment) => {
                let first = self.consume_keyword(Deployment)?;
                self.consume_keyword(Topology)?;
                (true, first)
            }
            _ => (false, self.consume_keyword(Topology)?),
        };
        let name = self.name()?;
        let implements = match self.peek(0) {
            Keyword(Implements) => {
                self.next();
                self.element_sequence(&Parser::qual_ident, Comma, LeftCurly)
            }
            _ => vec![],
        };

        self.consume(LeftCurly)?;
        let members = self.topology_members();
        self.consume(RightCurly)?;

        Ok(DefTopology {
            node_id: self.node(first.span()),
            is_deployment,
            name,
            members,
            implements,
        })
    }

    fn def_system(&mut self) -> ParseResult<DefSystem> {
        let first = self.consume_keyword(System)?;
        let name = self.name()?;
        self.consume(Colon)?;
        let topology = self.qual_ident()?;

        Ok(DefSystem {
            node_id: self.node(first.span()),
            name,
            topology,
        })
    }

    pub fn topology_members(&mut self) -> Vec<TopologyMember> {
        self.annotated_element_sequence(&Parser::topology_member, Semi, RightCurly)
    }

    fn topology_member(&mut self) -> ParseResult<TopologyMember> {
        match self.peek(0) {
            Keyword(Import) | Keyword(Instance) => {
                Ok(TopologyMember::SpecInstance(self.spec_instance()?))
            }
            Keyword(Include) => Ok(TopologyMember::SpecInclude(self.spec_include()?)),
            Keyword(Port) => Ok(TopologyMember::SpecTopPort(self.spec_top_port()?)),
            Keyword(Telemetry) => match self.peek(1) {
                Keyword(Packets) => Ok(TopologyMember::SpecTlmPacketSet(
                    self.spec_tlm_packet_set()?,
                )),
                _ => Ok(TopologyMember::SpecPatternConnectionGraph(
                    self.spec_connection_graph_pattern()?,
                )),
            },
            Keyword(Command) | Keyword(Event) | Keyword(Health) | Keyword(Param)
            | Keyword(Text) | Keyword(Time) => Ok(TopologyMember::SpecPatternConnectionGraph(
                self.spec_connection_graph_pattern()?,
            )),
            Keyword(Connections) => Ok(TopologyMember::SpecDirectConnectionGraph(
                self.spec_connection_graph_direct()?,
            )),
            _ => Err(self.cursor.err_expected_one_of(
                "topology member expected",
                vec![
                    Keyword(Instance),
                    Keyword(Include),
                    Keyword(Port),
                    Keyword(Telemetry),
                    Keyword(Command),
                    Keyword(Event),
                    Keyword(Health),
                    Keyword(Param),
                    Keyword(Text),
                    Keyword(Time),
                ],
            )),
        }
    }

    fn spec_connection_graph_pattern(&mut self) -> ParseResult<SpecPatternConnectionGraph> {
        let first_span = self.current_span()?;

        let kind = match self.peek(0) {
            Keyword(Command) => {
                self.next();
                ConnectionPatternKind::Command
            }
            Keyword(Event) => {
                self.next();
                ConnectionPatternKind::Event
            }
            Keyword(Health) => {
                self.next();
                ConnectionPatternKind::Health
            }
            Keyword(Param) => {
                self.next();
                ConnectionPatternKind::Param
            }
            Keyword(Telemetry) => {
                self.next();
                ConnectionPatternKind::Telemetry
            }
            Keyword(Text) => {
                self.next();
                self.consume_keyword(Event)?;
                ConnectionPatternKind::TextEvent
            }
            Keyword(Time) => {
                self.next();
                ConnectionPatternKind::Time
            }
            _ => {
                return Err(self.cursor.err_expected_one_of(
                    "connection pattern graph kind expected",
                    vec![
                        Keyword(Telemetry),
                        Keyword(Command),
                        Keyword(Event),
                        Keyword(Health),
                        Keyword(Param),
                        Keyword(Text),
                        Keyword(Time),
                    ],
                ));
            }
        };

        self.consume_keyword(Connections)?;
        self.consume_keyword(Instance)?;
        let source = self.qual_ident()?;

        let targets = match self.peek(0) {
            LeftCurly => {
                self.next();
                let out = self.element_sequence(&Parser::qual_ident, Comma, RightCurly);
                self.consume(RightCurly)?;
                out
            }
            _ => vec![],
        };

        Ok(SpecPatternConnectionGraph {
            node_id: self.node(first_span),
            kind,
            source,
            targets,
        })
    }

    fn spec_connection_graph_direct(&mut self) -> ParseResult<SpecDirectConnectionGraph> {
        let first = self.consume_keyword(Connections)?;
        let name = self.name()?;
        self.consume(LeftCurly)?;
        let connections = self.element_sequence(&Parser::connection, Comma, RightCurly);
        self.consume(RightCurly)?;

        Ok(SpecDirectConnectionGraph {
            node_id: self.node(first.span()),
            name,
            connections,
        })
    }

    fn connection(&mut self) -> ParseResult<Connection> {
        let first_span = self.current_span()?;

        let is_unmatched = match self.peek(0) {
            Keyword(Unmatched) => {
                self.next();
                true
            }
            _ => false,
        };

        let from_port = self.port_instance_identifier()?;
        let from_index = match self.peek(0) {
            LeftSquare => Some(self.index()?),
            _ => None,
        };

        self.consume(RightArrow)?;

        let to_port = self.port_instance_identifier()?;
        let to_index = match self.peek(0) {
            LeftSquare => Some(self.index()?),
            _ => None,
        };

        Ok(Connection {
            node_id: self.node(first_span),
            is_unmatched,
            from_port,
            from_index,
            to_port,
            to_index,
        })
    }

    fn spec_instance(&mut self) -> ParseResult<SpecInstance> {
        let first = match self.peek(0) {
            Keyword(Import) | Keyword(Instance) => self.next().unwrap(),
            _ => {
                return Err(self.cursor.err_expected_one_of(
                    "instance specifier expected",
                    vec![Keyword(Import), Keyword(Instance)],
                ));
            }
        };

        let instance = self.qual_ident()?;
        Ok(SpecInstance {
            node_id: self.node(first.span()),
            instance,
        })
    }

    fn spec_tlm_packet_set(&mut self) -> ParseResult<SpecTlmPacketSet> {
        let first = self.consume_keyword(Telemetry)?;
        self.consume_keyword(Packets)?;
        let name = self.name()?;
        self.consume(LeftCurly)?;
        let members = self.tlm_packet_set_members();
        self.consume(RightCurly)?;

        let omitted = match self.peek(0) {
            Keyword(Omit) => {
                self.next();
                self.consume(LeftCurly)?;
                let omit =
                    self.element_sequence(&Parser::tlm_channel_identifier, Comma, RightCurly);
                self.consume(RightCurly)?;
                omit
            }
            _ => vec![],
        };

        Ok(SpecTlmPacketSet {
            node_id: self.node(first.span()),
            name,
            members,
            omitted,
        })
    }

    pub fn tlm_packet_set_members(&mut self) -> Vec<TlmPacketSetMember> {
        self.annotated_element_sequence(&Parser::tlm_packet_set_member, Comma, RightCurly)
    }

    fn tlm_packet_set_member(&mut self) -> ParseResult<TlmPacketSetMember> {
        match self.peek(0) {
            Keyword(Include) => Ok(TlmPacketSetMember::SpecInclude(self.spec_include()?)),
            Keyword(Packet) => Ok(TlmPacketSetMember::SpecTlmPacket(self.spec_tlm_packet()?)),
            _ => Err(self.cursor.err_expected_one_of(
                "telemetry packet set member expected",
                vec![Keyword(Include), Keyword(Packet)],
            )),
        }
    }

    fn spec_tlm_packet(&mut self) -> ParseResult<SpecTlmPacket> {
        let first = self.consume_keyword(Packet)?;
        let name = self.name()?;
        let id = self.opt_expr(Id)?;
        self.consume_keyword(Group)?;
        let group = self.expr()?;
        self.consume(LeftCurly)?;
        let members = self.tlm_packet_members();
        self.consume(RightCurly)?;

        Ok(SpecTlmPacket {
            node_id: self.node(first.span()),
            name,
            id,
            group,
            members,
        })
    }

    pub fn tlm_packet_members(&mut self) -> Vec<TlmPacketMember> {
        self.element_sequence(&Parser::tlm_packet_member, Comma, RightCurly)
    }

    fn tlm_packet_member(&mut self) -> ParseResult<TlmPacketMember> {
        match self.peek(0) {
            Keyword(Include) => Ok(TlmPacketMember::SpecInclude(self.spec_include()?)),
            Identifier => Ok(TlmPacketMember::TlmChannelIdentifier(
                self.tlm_channel_identifier()?,
            )),
            _ => Err(self.cursor.err_expected_one_of(
                "telemetry packet member expected",
                vec![Keyword(Include), Identifier],
            )),
        }
    }

    fn tlm_channel_identifier(&mut self) -> ParseResult<TlmChannelIdentifier> {
        let (component_instance, channel_name, first_span) = self.interface_instance_member()?;

        Ok(TlmChannelIdentifier {
            node_id: self.node(first_span),
            component_instance,
            channel_name,
        })
    }

    fn port_instance_identifier(&mut self) -> ParseResult<PortInstanceIdentifier> {
        let (interface_instance, port_name, first_span) = self.interface_instance_member()?;

        Ok(PortInstanceIdentifier {
            node_id: self.node(first_span),
            interface_instance,
            port_name,
        })
    }

    fn interface_instance_member(&mut self) -> ParseResult<(QualIdent, Ident, fpp_core::Span)> {
        let first = self.ident()?;
        let first_span = first.span();

        let mut identifiers = vec![];
        self.consume(Dot)?;
        identifiers.push(self.ident()?);
        while self.peek(0) == Dot {
            self.next();
            identifiers.push(self.ident()?);
        }

        let member = identifiers.pop().unwrap();
        let instance = identifiers
            .into_iter()
            .fold(QualIdent::Unqualified(first), |q, ident| {
                let q_span = q.span();
                QualIdent::Qualified(Qualified {
                    node_id: self.node(q_span),
                    qualifier: Box::new(q),
                    name: ident,
                })
            });

        Ok((instance, member, first_span))
    }

    fn spec_top_port(&mut self) -> ParseResult<SpecTopPort> {
        let first = self.consume_keyword(Port)?;
        let name = self.name()?;
        self.consume(Equals)?;
        let underlying_port = self.port_instance_identifier()?;

        Ok(SpecTopPort {
            node_id: self.node(first.span()),
            name,
            underlying_port,
        })
    }

    fn def_component_instance(&mut self) -> ParseResult<DefComponentInstance> {
        let first = self.consume_keyword(Instance)?;
        let name = self.name()?;
        self.consume(Colon)?;
        let component = self.qual_ident()?;

        let base_id = {
            match self.peek(0) {
                Keyword(Base) => {
                    self.next();
                    self.consume_keyword(Id)?;
                    Some(self.expr()?)
                }
                _ => None,
            }
        };

        let impl_type = {
            match self.peek(0) {
                Keyword(Type) => {
                    self.next();
                    Some(self.lit_string()?)
                }
                _ => None,
            }
        };

        let file = {
            match self.peek(0) {
                Keyword(At) => {
                    self.next();
                    Some(self.lit_string()?)
                }
                _ => None,
            }
        };

        let queue_size = {
            match self.peek(0) {
                Keyword(Queue) => {
                    self.next();
                    self.consume_keyword(Size)?;
                    Some(self.expr()?)
                }
                _ => None,
            }
        };

        let stack_size = {
            match self.peek(0) {
                Keyword(Stack) => {
                    self.next();
                    self.consume_keyword(Size)?;
                    Some(self.expr()?)
                }
                _ => None,
            }
        };

        let priority = self.opt_expr(Priority)?;
        let cpu = self.opt_expr(Cpu)?;

        let init_specs = {
            match self.peek(0) {
                LeftCurly => {
                    self.next();
                    let seq = self.annotated_element_sequence(&Parser::spec_init, Semi, RightCurly);
                    self.consume(RightCurly)?;
                    seq
                }
                _ => vec![],
            }
        };

        Ok(DefComponentInstance {
            node_id: self.node(first.span()),
            name,
            component,
            base_id,
            impl_type,
            file,
            queue_size,
            stack_size,
            priority,
            cpu,
            init_specs,
        })
    }

    fn spec_init(&mut self) -> ParseResult<SpecInit> {
        let first = self.consume_keyword(Phase)?;
        let phase = self.expr()?;
        let code = self.lit_string()?;

        Ok(SpecInit {
            node_id: self.node(first.span()),
            phase,
            code,
        })
    }

    fn def_interface(&mut self) -> ParseResult<DefInterface> {
        let first = self.consume_keyword(Interface)?;
        let name = self.name()?;
        self.consume(LeftCurly)?;
        let members = self.annotated_element_sequence(&Parser::interface_member, Semi, RightCurly);
        self.consume(RightCurly)?;

        Ok(DefInterface {
            node_id: self.node(first.span()),
            name,
            members,
        })
    }

    fn interface_member(&mut self) -> ParseResult<InterfaceMember> {
        match self.peek(0) {
            Keyword(Import) => Ok(InterfaceMember::SpecInterfaceImport(
                self.spec_import_interface()?,
            )),
            _ => Ok(InterfaceMember::SpecPortInstance(
                self.spec_port_instance()?,
            )),
        }
    }

    fn def_constant(&mut self) -> ParseResult<DefConstant> {
        let (first, is_dictionary_def) = self.dictionary_opt(Constant)?;
        let name = self.name()?;

        self.consume(Equals)?;
        let value = self.expr()?;

        Ok(DefConstant {
            node_id: self.node(first.span()),
            name,
            value,
            is_dictionary_def,
        })
    }

    fn def_enum(&mut self) -> ParseResult<DefEnum> {
        let (first, is_dictionary_def) = self.dictionary_opt(Enum)?;
        let name = self.name()?;

        let type_name = match self.peek(0) {
            Colon => {
                self.next();
                Some(self.type_name()?)
            }
            _ => None,
        };

        self.consume(LeftCurly)?;
        let constants =
            self.annotated_element_sequence(&Parser::def_enum_constant, Comma, RightCurly);
        self.consume(RightCurly)?;

        let default = match self.peek(0) {
            Keyword(Default) => {
                self.next();
                Some(self.expr()?)
            }
            _ => None,
        };

        Ok(DefEnum {
            node_id: self.node(first.span()),
            name,
            type_name,
            constants,
            default,
            is_dictionary_def,
        })
    }

    fn def_enum_constant(&mut self) -> ParseResult<DefEnumConstant> {
        let name = self.name()?;
        let first_span = name.span();

        let value = match self.peek(0) {
            Equals => {
                self.next();
                Some(self.expr()?)
            }
            _ => None,
        };

        Ok(DefEnumConstant {
            node_id: self.node(first_span),
            name,
            value,
        })
    }

    fn spec_state_machine_instance(&mut self) -> ParseResult<SpecStateMachineInstance> {
        let first = self.consume_keyword(State)?;
        self.consume_keyword(Machine)?;
        self.consume_keyword(Instance)?;

        let name = self.name()?;

        self.consume(Colon)?;
        let state_machine = self.qual_ident()?;

        let priority = self.opt_expr(Priority)?;

        let queue_full = self.opt_queue_full_kind()?;

        Ok(SpecStateMachineInstance {
            node_id: self.node(first.span()),
            name,
            state_machine,
            priority,
            queue_full,
        })
    }

    fn def_state_machine(&mut self) -> ParseResult<DefStateMachine> {
        let first = self.consume_keyword(State)?;
        self.consume_keyword(Machine)?;

        let name = self.name()?;
        let members = match self.peek(0) {
            LeftCurly => {
                self.next();
                let out = self.annotated_element_sequence(
                    &Parser::state_machine_member,
                    Semi,
                    RightCurly,
                );

                self.consume(RightCurly)?;

                Some(out)
            }
            _ => None,
        };

        Ok(DefStateMachine {
            node_id: self.node(first.span()),
            name,
            members,
        })
    }

    fn state_machine_member(&mut self) -> ParseResult<StateMachineMember> {
        match self.peek(0) {
            Keyword(Type) => match self.peek(2) {
                Equals => Ok(StateMachineMember::DefAliasType(self.alias_type()?)),
                _ => Ok(StateMachineMember::DefAbsType(self.abs_type()?)),
            },
            Keyword(Array) => Ok(StateMachineMember::DefArray(self.def_array()?)),
            Keyword(Constant) => Ok(StateMachineMember::DefConstant(self.def_constant()?)),
            Keyword(Enum) => Ok(StateMachineMember::DefEnum(self.def_enum()?)),
            Keyword(Struct) => Ok(StateMachineMember::DefStruct(self.def_struct()?)),
            Keyword(Include) => Ok(StateMachineMember::SpecInclude(self.spec_include()?)),
            Keyword(Initial) => Ok(StateMachineMember::SpecInitialTransition(
                self.spec_initial_transition()?,
            )),
            Keyword(State) => Ok(StateMachineMember::DefState(self.def_state()?)),
            Keyword(Signal) => Ok(StateMachineMember::DefSignal(self.def_signal()?)),
            Keyword(Action) => Ok(StateMachineMember::DefAction(self.def_action()?)),
            Keyword(Guard) => Ok(StateMachineMember::DefGuard(self.def_guard()?)),
            Keyword(Choice) => Ok(StateMachineMember::DefChoice(self.def_choice()?)),
            _ => Err(self.cursor.err_expected_one_of(
                "state machine member expected",
                vec![
                    Keyword(Type),
                    Keyword(Array),
                    Keyword(Constant),
                    Keyword(Enum),
                    Keyword(Struct),
                    Keyword(Include),
                    Keyword(Initial),
                    Keyword(State),
                    Keyword(Signal),
                    Keyword(Action),
                    Keyword(Guard),
                    Keyword(Choice),
                ],
            )),
        }
    }

    fn state_member(&mut self) -> ParseResult<StateMember> {
        match self.peek(0) {
            Keyword(Choice) => Ok(StateMember::DefChoice(self.def_choice()?)),
            Keyword(State) => Ok(StateMember::DefState(self.def_state()?)),
            Keyword(Initial) => Ok(StateMember::SpecInitialTransition(
                self.spec_initial_transition()?,
            )),
            Keyword(Entry) => Ok(StateMember::SpecStateEntry(self.spec_state_entry()?)),
            Keyword(Exit) => Ok(StateMember::SpecStateExit(self.spec_state_exit()?)),
            Keyword(Include) => Ok(StateMember::SpecInclude(self.spec_include()?)),
            Keyword(On) => Ok(StateMember::SpecStateTransition(
                self.spec_state_transition()?,
            )),
            _ => Err(self.cursor.err_expected_one_of(
                "state member expected",
                vec![
                    Keyword(Choice),
                    Keyword(State),
                    Keyword(Initial),
                    Keyword(Entry),
                    Keyword(Exit),
                    Keyword(Include),
                    Keyword(On),
                ],
            )),
        }
    }

    fn spec_initial_transition(&mut self) -> ParseResult<SpecInitialTransition> {
        let first = self.consume_keyword(Initial)?;
        let transition = self.transition_expr()?;

        Ok(SpecInitialTransition {
            node_id: self.node(first.span()),
            transition,
        })
    }

    fn spec_state_entry(&mut self) -> ParseResult<SpecStateEntry> {
        let first = self.consume_keyword(Entry)?;
        let actions = self.do_expr()?;

        Ok(SpecStateEntry {
            node_id: self.node(first.span()),
            actions,
        })
    }

    fn do_expr(&mut self) -> ParseResult<DoExpr> {
        let first = self.consume_keyword(Do)?;
        self.consume(LeftCurly)?;
        let actions = self.element_sequence(&Parser::ident, Comma, RightCurly);
        self.consume(RightCurly)?;

        Ok(DoExpr {
            node_id: self.node(first.span()),
            actions,
        })
    }

    fn spec_state_exit(&mut self) -> ParseResult<SpecStateExit> {
        let first = self.consume_keyword(Exit)?;
        let actions = self.do_expr()?;
        Ok(SpecStateExit {
            node_id: self.node(first.span()),
            actions,
        })
    }

    fn spec_state_transition(&mut self) -> ParseResult<SpecStateTransition> {
        let first = self.consume_keyword(On)?;
        let signal = self.ident()?;
        let guard = match self.peek(0) {
            Keyword(If) => {
                self.next();
                Some(self.ident()?)
            }
            _ => None,
        };

        let transition_or_do = self.transition_or_do()?;

        Ok(SpecStateTransition {
            node_id: self.node(first.span()),
            signal,
            guard,
            transition_or_do,
        })
    }

    fn transition_or_do(&mut self) -> ParseResult<TransitionOrDo> {
        let first_span = self.current_span()?;

        let do_expr = match self.peek(0) {
            Keyword(Do) => Some(self.do_expr()?),
            _ => None,
        };

        match self.peek(0) {
            Keyword(Enter) => {
                self.next();
                let target = self.qual_ident()?;
                Ok(TransitionOrDo::Transition(TransitionExpr {
                    node_id: self.node(first_span),
                    actions: do_expr,
                    target,
                }))
            }
            _ => match do_expr {
                Some(de) => Ok(TransitionOrDo::Do(de)),
                None => Err(self.cursor.err_expected_one_of(
                    "expected transition or do",
                    vec![Keyword(Do), Keyword(Enter)],
                )),
            },
        }
    }

    fn transition_expr(&mut self) -> ParseResult<TransitionExpr> {
        let first_span = self.current_span()?;

        let do_expr = match self.peek(0) {
            Keyword(Do) => Some(self.do_expr()?),
            _ => None,
        };

        match self.peek(0) {
            Keyword(Enter) => {
                self.next();
                let target = self.qual_ident()?;
                Ok(TransitionExpr {
                    node_id: self.node(first_span),
                    actions: do_expr,
                    target,
                })
            }
            _ => Err(self.cursor.err_expected_one_of(
                "expected transition expression",
                vec![Keyword(Enter), Keyword(Do)],
            )),
        }
    }

    fn def_guard(&mut self) -> ParseResult<DefGuard> {
        let first = self.consume_keyword(Guard)?;
        let name = self.name()?;
        let type_name = match self.peek(0) {
            Colon => {
                self.next();
                Some(self.type_name()?)
            }
            _ => None,
        };

        Ok(DefGuard {
            node_id: self.node(first.span()),
            name,
            type_name,
        })
    }

    fn spec_container(&mut self) -> ParseResult<SpecContainer> {
        let first = self.consume_keyword(Product)?;
        self.consume_keyword(Container)?;
        let name = self.name()?;
        let id = self.opt_expr(Id)?;
        let default_priority = match self.peek(0) {
            Keyword(Default) => {
                self.next();
                self.consume_keyword(Priority)?;
                Some(self.expr()?)
            }
            _ => None,
        };

        Ok(SpecContainer {
            node_id: self.node(first.span()),
            name,
            id,
            default_priority,
        })
    }

    fn spec_record(&mut self) -> ParseResult<SpecRecord> {
        let first = self.consume_keyword(Product)?;
        self.consume_keyword(Record)?;
        let name = self.name()?;
        self.consume(Colon)?;
        let record_type = self.type_name()?;
        let is_array = match self.peek(0) {
            Keyword(Array) => {
                self.next();
                true
            }
            _ => false,
        };

        let id = self.opt_expr(Id)?;
        Ok(SpecRecord {
            node_id: self.node(first.span()),
            name,
            record_type,
            is_array,
            id,
        })
    }

    fn spec_event(&mut self) -> ParseResult<SpecEvent> {
        let first = self.consume_keyword(Event)?;
        let name = self.name()?;
        let params = self.formal_param_list()?;
        self.consume_keyword(Severity)?;
        let severity = match self.peek(0) {
            Keyword(Activity) => {
                self.next();
                match self.peek(0) {
                    Keyword(High) => {
                        self.next();
                        Ok(EventSeverity::ActivityHigh)
                    }
                    Keyword(Low) => {
                        self.next();
                        Ok(EventSeverity::ActivityLow)
                    }
                    _ => Err(self.cursor.err_expected_one_of(
                        "severity level expected",
                        vec![Keyword(High), Keyword(Low)],
                    )),
                }
            }
            Keyword(Warning) => {
                self.next();
                match self.peek(0) {
                    Keyword(High) => {
                        self.next();
                        Ok(EventSeverity::WarningHigh)
                    }
                    Keyword(Low) => {
                        self.next();
                        Ok(EventSeverity::WarningLow)
                    }
                    _ => Err(self.cursor.err_expected_one_of(
                        "severity level expected",
                        vec![Keyword(High), Keyword(Low)],
                    )),
                }
            }
            Keyword(Command) => {
                self.next();
                Ok(EventSeverity::Command)
            }
            Keyword(Diagnostic) => {
                self.next();
                Ok(EventSeverity::Diagnostic)
            }
            Keyword(Fatal) => {
                self.next();
                Ok(EventSeverity::Fatal)
            }
            _ => Err(self.cursor.err_expected_one_of(
                "severity level expected",
                vec![
                    Keyword(Activity),
                    Keyword(Warning),
                    Keyword(Command),
                    Keyword(Diagnostic),
                    Keyword(Fatal),
                ],
            )),
        }?;

        let id = self.opt_expr(Id)?;
        self.consume_keyword(Format)?;
        let format = self.lit_string()?;
        let throttle = match self.peek(0) {
            Keyword(Throttle) => Some(self.event_throttle()?),
            _ => None,
        };

        Ok(SpecEvent {
            node_id: self.node(first.span()),
            name,
            params,
            severity,
            id,
            format,
            throttle,
        })
    }

    fn event_throttle(&mut self) -> ParseResult<EventThrottle> {
        let first = self.consume_keyword(Throttle)?;
        let count = self.expr()?;
        let every = self.opt_expr(Every)?;

        Ok(EventThrottle {
            node_id: self.node(first.span()),
            count,
            every,
        })
    }

    fn spec_include(&mut self) -> ParseResult<SpecInclude> {
        let first = self.consume_keyword(Include)?;
        let file = self.lit_string()?;
        Ok(SpecInclude {
            node_id: self.node(first.span()),
            file,
        })
    }

    fn spec_import_interface(&mut self) -> ParseResult<SpecInterfaceImport> {
        let first = self.consume_keyword(Import)?;
        let interface = self.qual_ident()?;
        Ok(SpecInterfaceImport {
            node_id: self.node(first.span()),
            interface,
        })
    }

    fn spec_internal_port(&mut self) -> ParseResult<SpecInternalPort> {
        let first = self.consume_keyword(Internal)?;
        self.consume_keyword(Port)?;
        let name = self.name()?;
        let params = self.formal_param_list()?;
        let priority = self.opt_expr(Priority)?;
        let queue_full = self.opt_queue_full_kind()?;
        Ok(SpecInternalPort {
            node_id: self.node(first.span()),
            name,
            params,
            priority,
            queue_full,
        })
    }

    fn spec_port_matching(&mut self) -> ParseResult<SpecPortMatching> {
        let first = self.consume_keyword(Match)?;
        let port1 = self.ident()?;
        self.consume_keyword(With)?;
        let port2 = self.ident()?;
        Ok(SpecPortMatching {
            node_id: self.node(first.span()),
            port1,
            port2,
        })
    }

    fn spec_param(&mut self) -> ParseResult<SpecParam> {
        let first_span = self.current_span()?;
        let is_external = match self.peek(0) {
            Keyword(External) => {
                self.next();
                true
            }
            _ => false,
        };

        self.consume_keyword(Param)?;
        let name = self.name()?;
        self.consume(Colon)?;
        let type_name = self.type_name()?;
        let default = self.opt_expr(Default)?;
        let id = self.opt_expr(Id)?;

        let set_opcode = match self.peek(0) {
            Keyword(Set) => {
                self.next();
                self.consume_keyword(Opcode)?;
                Some(self.expr()?)
            }
            _ => None,
        };

        let save_opcode = match self.peek(0) {
            Keyword(Save) => {
                self.next();
                self.consume_keyword(Opcode)?;
                Some(self.expr()?)
            }
            _ => None,
        };

        Ok(SpecParam {
            node_id: self.node(first_span),
            name,
            type_name,
            default,
            id,
            set_opcode,
            save_opcode,
            is_external,
        })
    }

    fn spec_tlm_channel(&mut self) -> ParseResult<SpecTlmChannel> {
        let first = self.consume_keyword(Telemetry)?;
        let name = self.name()?;
        self.consume(Colon)?;
        let type_name = self.type_name()?;
        let id = self.opt_expr(Id)?;
        let update = match self.peek(0) {
            Keyword(Update) => {
                self.next();
                match self.peek(0) {
                    Keyword(Always) => {
                        self.next();
                        Ok(Some(TlmChannelUpdate::Always))
                    }
                    Keyword(On) => {
                        self.next();
                        self.consume_keyword(Change)?;
                        Ok(Some(TlmChannelUpdate::OnChange))
                    }
                    _ => Err(self.cursor.err_expected_one_of(
                        "update kind expected",
                        vec![Keyword(Always), Keyword(On)],
                    )),
                }
            }
            _ => Ok(None),
        }?;

        let format = match self.peek(0) {
            Keyword(Format) => {
                self.next();
                Some(self.lit_string()?)
            }
            _ => None,
        };

        let low = match self.peek(0) {
            Keyword(Low) => {
                self.next();
                self.limit_sequence()?
            }
            _ => vec![],
        };
        let high = match self.peek(0) {
            Keyword(High) => {
                self.next();
                self.limit_sequence()?
            }
            _ => vec![],
        };

        Ok(SpecTlmChannel {
            node_id: self.node(first.span()),
            name,
            type_name,
            id,
            update,
            format,
            low,
            high,
        })
    }

    fn limit_sequence(&mut self) -> ParseResult<Vec<TlmChannelLimit>> {
        self.consume(LeftCurly)?;
        let out = self.element_sequence(&Parser::limit, Comma, RightCurly);
        self.consume(RightCurly)?;
        Ok(out)
    }

    fn limit(&mut self) -> ParseResult<TlmChannelLimit> {
        let first_span = self.current_span()?;
        let kind = self.limit_kind()?;
        let value = self.expr()?;
        Ok(TlmChannelLimit {
            node_id: self.node(first_span),
            kind,
            value,
        })
    }

    fn limit_kind(&mut self) -> ParseResult<TlmChannelLimitKind> {
        let kind = match self.peek(0) {
            Keyword(Orange) => {
                self.next();
                Ok(TlmChannelLimitKind::Orange)
            }
            Keyword(Red) => {
                self.next();
                Ok(TlmChannelLimitKind::Red)
            }
            Keyword(Yellow) => {
                self.next();
                Ok(TlmChannelLimitKind::Yellow)
            }
            _ => Err(self.cursor.err_expected_one_of(
                "telemetry channel limit kind expected",
                vec![Keyword(Orange), Keyword(Red), Keyword(Yellow)],
            )),
        }?;

        Ok(kind)
    }

    fn def_struct(&mut self) -> ParseResult<DefStruct> {
        let (first, is_dictionary_def) = self.dictionary_opt(Struct)?;
        let name = self.name()?;

        self.consume(LeftCurly)?;
        let members =
            self.annotated_element_sequence(&Parser::struct_type_member, Comma, RightCurly);
        self.consume(RightCurly)?;

        let default = match self.peek(0) {
            Keyword(Default) => {
                self.next();
                Some(self.expr()?)
            }
            _ => None,
        };

        Ok(DefStruct {
            node_id: self.node(first.span()),
            name,
            members,
            default,
            is_dictionary_def,
        })
    }

    fn struct_type_member(&mut self) -> ParseResult<StructTypeMember> {
        let name = self.name()?;
        let first_span = name.span();

        self.consume(Colon)?;
        let size = match self.peek(0) {
            LeftSquare => Some(self.index()?),
            _ => None,
        };

        let type_name = self.type_name()?;
        let format = match self.peek(0) {
            Keyword(Format) => {
                self.next();
                Some(self.lit_string()?)
            }
            _ => None,
        };

        Ok(StructTypeMember {
            node_id: self.node(first_span),
            name,
            size,
            type_name,
            format,
        })
    }

    fn def_state(&mut self) -> ParseResult<DefState> {
        let first = self.consume_keyword(State)?;
        let name = self.name()?;

        let members = match self.peek(0) {
            LeftCurly => {
                self.next();
                let members =
                    self.annotated_element_sequence(&Parser::state_member, Semi, RightCurly);
                self.consume(RightCurly)?;
                members
            }
            _ => vec![],
        };

        Ok(DefState {
            node_id: self.node(first.span()),
            name,
            members,
        })
    }

    fn def_signal(&mut self) -> ParseResult<DefSignal> {
        let first = self.consume_keyword(Signal)?;
        let name = self.name()?;
        let type_name = match self.peek(0) {
            Colon => {
                self.next();
                Some(self.type_name()?)
            }
            _ => None,
        };

        Ok(DefSignal {
            node_id: self.node(first.span()),
            name,
            type_name,
        })
    }

    fn spec_port_instance_general(&mut self) -> ParseResult<SpecPortInstance> {
        let first = match self.peek_span(0) {
            Some(span) => span,
            None => {
                return Err(self.cursor.err_expected_one_of(
                    "general port expected",
                    vec![
                        Keyword(Async),
                        Keyword(Sync),
                        Keyword(Guarded),
                        Keyword(Output),
                    ],
                ));
            }
        };

        let kind = match self.peek(0) {
            Keyword(Async) => {
                self.next();
                self.consume_keyword(Input)?;
                Ok(GeneralPortInstanceKind::Input(InputPortKind::Async))
            }
            Keyword(Guarded) => {
                self.next();
                self.consume_keyword(Input)?;
                Ok(GeneralPortInstanceKind::Input(InputPortKind::Guarded))
            }
            Keyword(Sync) => {
                self.next();
                self.consume_keyword(Input)?;
                Ok(GeneralPortInstanceKind::Input(InputPortKind::Sync))
            }
            Keyword(Output) => {
                self.next();
                Ok(GeneralPortInstanceKind::Output)
            }
            _ => Err(self.cursor.err_expected_one_of(
                "expected general port instance",
                vec![
                    Keyword(Async),
                    Keyword(Guarded),
                    Keyword(Sync),
                    Keyword(Output),
                ],
            )),
        }?;

        self.consume_keyword(Port)?;
        let name = self.name()?;

        self.consume(Colon)?;
        let size = match self.peek(0) {
            LeftSquare => Some(self.index()?),
            _ => None,
        };

        let port = match self.peek(0) {
            Keyword(Serial) => {
                self.consume_keyword(Serial)?;
                None
            }
            _ => Some(self.qual_ident()?),
        };

        let priority = match self.peek(0) {
            Keyword(Priority) => {
                self.next();
                Some(self.expr()?)
            }
            _ => None,
        };

        let queue_full = self.opt_queue_full()?;

        Ok(SpecPortInstance::General(SpecGeneralPortInstance {
            node_id: self.node(first),
            kind,
            name,
            size,
            port,
            priority,
            queue_full,
        }))
    }

    fn spec_port_instance_special(&mut self) -> ParseResult<SpecPortInstance> {
        let first = match self.peek_span(0) {
            Some(span) => span,
            None => {
                return Err(self.cursor.err_expected_one_of(
                    "special port expected",
                    vec![
                        Keyword(Async),
                        Keyword(Sync),
                        Keyword(Guarded),
                        Keyword(Command),
                        Keyword(Event),
                        Keyword(Param),
                        Keyword(Product),
                        Keyword(Telemetry),
                        Keyword(Text),
                        Keyword(Time),
                    ],
                ));
            }
        };

        let input_kind = match self.peek(0) {
            Keyword(Async) => {
                self.next();
                Some(InputPortKind::Async)
            }
            Keyword(Sync) => {
                self.next();
                Some(InputPortKind::Sync)
            }
            Keyword(Guarded) => {
                self.next();
                Some(InputPortKind::Guarded)
            }
            _ => None,
        };

        let kind = match self.peek(0) {
            Keyword(Command) => match self.peek(1) {
                Keyword(Recv) => {
                    self.next();
                    self.next();
                    Ok(SpecialPortInstanceKind::CommandRecv)
                }
                Keyword(Reg) => {
                    self.next();
                    self.next();
                    Ok(SpecialPortInstanceKind::CommandReg)
                }
                Keyword(Resp) => {
                    self.next();
                    self.next();
                    Ok(SpecialPortInstanceKind::CommandResp)
                }
                _ => Err(self.cursor.err_expected_one_of(
                    "command special port expected",
                    vec![Keyword(Recv), Keyword(Reg), Keyword(Resp)],
                )),
            },
            Keyword(Event) => {
                self.next();
                Ok(SpecialPortInstanceKind::Event)
            }
            Keyword(Param) => match self.peek(1) {
                Keyword(Get) => {
                    self.next();
                    self.next();
                    Ok(SpecialPortInstanceKind::ParamGet)
                }
                Keyword(Set) => {
                    self.next();
                    self.next();
                    Ok(SpecialPortInstanceKind::ParamSet)
                }
                _ => Err(self.cursor.err_expected_one_of(
                    "param special port expected",
                    vec![Keyword(Get), Keyword(Set)],
                )),
            },
            Keyword(Product) => match self.peek(1) {
                Keyword(Get) => {
                    self.next();
                    self.next();
                    Ok(SpecialPortInstanceKind::ProductGet)
                }
                Keyword(Recv) => {
                    self.next();
                    self.next();
                    Ok(SpecialPortInstanceKind::ProductRecv)
                }
                Keyword(Request) => {
                    self.next();
                    self.next();
                    Ok(SpecialPortInstanceKind::ProductRequest)
                }
                Keyword(Send) => {
                    self.next();
                    self.next();
                    Ok(SpecialPortInstanceKind::ProductSend)
                }
                _ => Err(self.cursor.err_expected_one_of(
                    "product special port expected",
                    vec![Keyword(Get), Keyword(Recv), Keyword(Request), Keyword(Send)],
                )),
            },
            Keyword(Telemetry) => {
                self.next();
                Ok(SpecialPortInstanceKind::Telemetry)
            }
            Keyword(Text) => {
                self.next();
                self.consume_keyword(Event)?;
                Ok(SpecialPortInstanceKind::TextEvent)
            }
            Keyword(Time) => {
                self.next();
                self.consume_keyword(Get)?;
                Ok(SpecialPortInstanceKind::TimeGet)
            }
            _ => Err(self.cursor.err_expected_one_of(
                "special port expected",
                vec![
                    Keyword(Command),
                    Keyword(Event),
                    Keyword(Param),
                    Keyword(Product),
                    Keyword(Telemetry),
                    Keyword(Text),
                    Keyword(Time),
                ],
            )),
        }?;

        self.consume_keyword(Port)?;
        let name = self.name()?;

        let priority = match self.peek(0) {
            Keyword(Priority) => {
                self.next();
                Some(self.expr()?)
            }
            _ => None,
        };

        let queue_full = self.opt_queue_full()?;

        Ok(SpecPortInstance::Special(SpecSpecialPortInstance {
            node_id: self.node(first),
            input_kind,
            kind,
            name,
            priority,
            queue_full,
        }))
    }

    fn spec_port_instance(&mut self) -> ParseResult<SpecPortInstance> {
        match self.peek(0) {
            Keyword(Async) | Keyword(Guarded) | Keyword(Sync) => match self.peek(1) {
                Keyword(Input) => self.spec_port_instance_general(),
                _ => self.spec_port_instance_special(),
            },
            Keyword(Output) => self.spec_port_instance_general(),
            _ => self.spec_port_instance_special(),
        }
    }

    fn spec_command(&mut self) -> ParseResult<SpecCommand> {
        let first_span = self.current_span()?;

        let kind = match self.peek(0) {
            Keyword(Async) => {
                self.next();
                self.consume_keyword(Command)?;
                Ok(InputPortKind::Async)
            }
            Keyword(Guarded) => {
                self.next();
                self.consume_keyword(Command)?;
                Ok(InputPortKind::Guarded)
            }
            Keyword(Sync) => {
                self.next();
                self.consume_keyword(Command)?;
                Ok(InputPortKind::Sync)
            }
            _ => Err(self.cursor.err_expected_one_of(
                "command kind expected",
                vec![Keyword(Async), Keyword(Guarded), Keyword(Sync)],
            )),
        }?;

        let name = self.name()?;
        let params = self.formal_param_list()?;
        let opcode = self.opt_expr(Opcode)?;
        let priority = self.opt_expr(Priority)?;
        let queue_full = self.opt_queue_full()?;

        Ok(SpecCommand {
            node_id: self.node(first_span),
            kind,
            name,
            params,
            opcode,
            priority,
            queue_full,
        })
    }

    /// Parses an optional queue full specifier node.
    fn opt_queue_full(&mut self) -> ParseResult<Option<QueueFullSpecifier>> {
        match self.peek(0) {
            Keyword(Assert) | Keyword(Block) | Keyword(Drop) | Keyword(Hook) => {
                Ok(Some(self.queue_full()?))
            }
            _ => Ok(None),
        }
    }

    /// Parses a queue full specifier node, whose span covers only the keyword
    /// that names the behavior.
    fn queue_full(&mut self) -> ParseResult<QueueFullSpecifier> {
        let first = self.current_span()?;
        let kind = self.queue_full_kind()?;
        Ok(QueueFullSpecifier {
            node_id: self.node(first),
            kind,
        })
    }

    /// Parses an optional bare queue full behavior, for the specifiers that do
    /// not wrap it in a node.
    fn opt_queue_full_kind(&mut self) -> ParseResult<Option<QueueFull>> {
        match self.peek(0) {
            Keyword(Assert) | Keyword(Block) | Keyword(Drop) | Keyword(Hook) => {
                Ok(Some(self.queue_full_kind()?))
            }
            _ => Ok(None),
        }
    }

    fn queue_full_kind(&mut self) -> ParseResult<QueueFull> {
        let out = match self.peek(0) {
            Keyword(Assert) => Ok(QueueFull::Assert),
            Keyword(Block) => Ok(QueueFull::Block),
            Keyword(Drop) => Ok(QueueFull::Drop),
            Keyword(Hook) => Ok(QueueFull::Hook),
            _ => Err(self.cursor.err_expected_one_of(
                "queue full expected",
                vec![
                    Keyword(Assert),
                    Keyword(Block),
                    Keyword(Drop),
                    Keyword(Hook),
                ],
            )),
        }?;

        self.next();
        Ok(out)
    }

    fn pre_annotation(&mut self) -> Vec<String> {
        let mut out: Vec<String> = vec![];

        while self.peek(0) == PreAnnotation {
            out.push(self.next().unwrap().text().to_string())
        }

        out
    }

    fn post_annotation(&mut self) -> Vec<String> {
        let mut out: Vec<String> = vec![];

        while self.peek(0) == PostAnnotation {
            out.push(self.next().unwrap().text().to_string())
        }

        out
    }

    fn annotated_element<T: fpp_core::Annotated + AstNode>(
        &mut self,
        element_parser: &dyn Fn(&mut Parser<'a>) -> ParseResult<T>,
        punct: &TokenKind,
        end: &TokenKind,
    ) -> ElementParsingResult<T> {
        let pre_annotation = self.pre_annotation();

        // Check if we reached the end
        let next_token = self.peek(0);
        if next_token == *end || next_token == EOF {
            // Stop parsing elements
            return ElementParsingResult::None;
        }

        let data = match element_parser(self) {
            Ok(data) => data,
            Err(err) => return ElementParsingResult::Err(err),
        };

        // Check if the punctuation exists
        let punct_tok = self.peek(0);
        if punct_tok == *punct || punct_tok == Eol || punct_tok == EOF {
            self.next();
            let post_annotation = self.post_annotation();
            fpp_core::Node::annotate(&data.id(), pre_annotation, post_annotation);
            ElementParsingResult::Terminated(data)
        } else if self.peek(0) == PostAnnotation {
            let post_annotation = self.post_annotation();
            fpp_core::Node::annotate(&data.id(), pre_annotation, post_annotation);
            ElementParsingResult::Terminated(data)
        } else if punct_tok == *end {
            fpp_core::Node::annotate(&data.id(), pre_annotation, vec![]);
            ElementParsingResult::Unterminated(data)
        } else {
            fpp_core::Node::annotate(&data.id(), pre_annotation, vec![]);
            ElementParsingResult::Err(self.cursor.err_expected_token(
                "unexpected token",
                *punct,
                punct_tok,
            ))
        }
    }

    #[inline]
    fn annotated_element_sequence<T: fpp_core::Annotated + AstNode>(
        &mut self,
        element_parser: &dyn Fn(&mut Parser<'a>) -> ParseResult<T>,
        punct: TokenKind,
        end: TokenKind,
    ) -> Vec<T> {
        // Eat up all the EOLs
        while self.peek(0) == Eol {
            self.next();
        }

        // Keep reading terminated elements until we can't
        let mut out: Vec<T> = vec![];

        loop {
            match self.annotated_element(element_parser, &punct, &end) {
                ElementParsingResult::Terminated(el) => {
                    out.push(el);
                }
                ElementParsingResult::Unterminated(el) => {
                    out.push(el);
                    break;
                }
                ElementParsingResult::None => {
                    break;
                }
                ElementParsingResult::Err(err) => {
                    fpp_core::Diagnostic::from(err).emit();

                    // Recover to an anchor
                    loop {
                        let current = self.peek(0);
                        if current == punct || current == Eol || current == EOF {
                            self.next();
                            break;
                        } else if current == end {
                            return out;
                        }

                        self.next();
                    }
                }
            }
        }

        out
    }

    fn element<T>(
        &mut self,
        element_parser: &dyn Fn(&mut Parser<'a>) -> ParseResult<T>,
        punct: &TokenKind,
        end: &TokenKind,
    ) -> ElementParsingResult<T> {
        // Check if we reached the end
        let next_token = self.peek(0);
        if next_token == *end || next_token == EOF {
            // Stop parsing elements
            return ElementParsingResult::None;
        }

        let data = match element_parser(self) {
            Ok(data) => data,
            Err(err) => return ElementParsingResult::Err(err),
        };

        // Check if the punctuation exists
        let punct_tok = self.peek(0);
        if punct_tok == *punct || punct_tok == Eol || punct_tok == EOF {
            self.next();
            ElementParsingResult::Terminated(data)
        } else if self.peek(0) == PostAnnotation {
            ElementParsingResult::Terminated(data)
        } else if punct_tok == *end {
            ElementParsingResult::Unterminated(data)
        } else {
            ElementParsingResult::Err(self.cursor.err_expected_token(
                "unexpected token",
                *punct,
                punct_tok,
            ))
        }
    }

    #[inline]
    fn element_sequence<T>(
        &mut self,
        element_parser: &dyn Fn(&mut Parser<'a>) -> ParseResult<T>,
        punct: TokenKind,
        end: TokenKind,
    ) -> Vec<T> {
        // Eat up all the EOLs
        while self.peek(0) == Eol {
            self.next();
        }

        // Keep reading terminated elements until we can't
        let mut out: Vec<T> = vec![];

        loop {
            match self.element(&element_parser, &punct, &end) {
                ElementParsingResult::Terminated(el) => {
                    out.push(el);
                }
                ElementParsingResult::Unterminated(el) => {
                    out.push(el);
                    break;
                }
                ElementParsingResult::None => {
                    break;
                }
                ElementParsingResult::Err(err) => {
                    fpp_core::Diagnostic::from(err).emit();

                    // Recover to an anchor
                    loop {
                        let current = self.peek(0);
                        if current == punct || current == Eol || current == EOF {
                            self.next();
                            break;
                        } else if current == end {
                            return out;
                        }

                        self.next();
                    }
                }
            }
        }

        out
    }

    fn index(&mut self) -> ParseResult<Expr> {
        self.consume(LeftSquare)?;
        let out = self.expr()?;
        self.consume(RightSquare)?;
        Ok(out)
    }

    fn qual_ident(&mut self) -> ParseResult<QualIdent> {
        let first = self.ident()?;
        let mut out = vec![];
        while self.peek(0) == Dot {
            let dot_token = self.next().unwrap();
            if self.peek(0) == Identifier {
                out.push(self.ident()?)
            } else {
                fpp_core::Diagnostic::from(ParseError::ExpectedToken {
                    expected: Identifier,
                    got: self.peek(0),
                    last: dot_token.span,
                    msg: "expected identifier",
                })
                .emit();
            }
        }

        Ok(out
            .into_iter()
            .fold(QualIdent::Unqualified(first), |q, ident| {
                let q_span = q.span();
                QualIdent::Qualified(Qualified {
                    node_id: self.node(q_span),
                    qualifier: Box::new(q),
                    name: ident,
                })
            }))
    }

    fn type_name(&mut self) -> ParseResult<TypeName> {
        let first_span = self.current_span()?;
        let kind = match self.peek(0) {
            Keyword(Bool) => {
                self.next();
                Ok(TypeNameKind::Bool)
            }
            Keyword(I8) => {
                self.next();
                Ok(TypeNameKind::Integer(IntegerKind::I8))
            }
            Keyword(U8) => {
                self.next();
                Ok(TypeNameKind::Integer(IntegerKind::U8))
            }
            Keyword(I16) => {
                self.next();
                Ok(TypeNameKind::Integer(IntegerKind::I16))
            }
            Keyword(U16) => {
                self.next();
                Ok(TypeNameKind::Integer(IntegerKind::U16))
            }
            Keyword(I32) => {
                self.next();
                Ok(TypeNameKind::Integer(IntegerKind::I32))
            }
            Keyword(U32) => {
                self.next();
                Ok(TypeNameKind::Integer(IntegerKind::U32))
            }
            Keyword(I64) => {
                self.next();
                Ok(TypeNameKind::Integer(IntegerKind::I64))
            }
            Keyword(U64) => {
                self.next();
                Ok(TypeNameKind::Integer(IntegerKind::U64))
            }
            Keyword(F32) => {
                self.next();
                Ok(TypeNameKind::Floating(FloatKind::F32))
            }
            Keyword(F64) => {
                self.next();
                Ok(TypeNameKind::Floating(FloatKind::F64))
            }
            Keyword(String_) => {
                self.next();
                let size = match self.peek(0) {
                    Keyword(Size) => {
                        self.next();
                        Some(self.expr()?)
                    }
                    _ => None,
                };
                Ok(TypeNameKind::String(size))
            }
            Identifier => Ok(TypeNameKind::QualIdent(self.qual_ident()?)),
            _ => Err(self.cursor.err_expected_one_of(
                "type name expected",
                vec![
                    Keyword(Bool),
                    Keyword(I8),
                    Keyword(U8),
                    Keyword(I16),
                    Keyword(U16),
                    Keyword(I32),
                    Keyword(U32),
                    Keyword(I64),
                    Keyword(U64),
                    Keyword(F32),
                    Keyword(F64),
                    Keyword(String_),
                    Identifier,
                ],
            )),
        }?;

        Ok(TypeName {
            node_id: self.node(first_span),
            kind,
        })
    }

    fn expr(&mut self) -> ParseResult<Expr> {
        let mut left = self.expr_shift_operand()?;
        let first_span = left.span();

        loop {
            let op = match self.peek(0) {
                ShiftLeft => Some(Binop::LShift),
                ShiftRight => Some(Binop::RShift),
                _ => None,
            };

            match op {
                Some(op) => {
                    self.next();
                    let right = self.expr_shift_operand()?;
                    left = Expr {
                        node_id: self.node(first_span),
                        kind: ExprKind::Binop {
                            left: Box::new(left),
                            op,
                            right: Box::new(right),
                        },
                    }
                }
                None => return Ok(left),
            }
        }
    }

    fn expr_shift_operand(&mut self) -> ParseResult<Expr> {
        let mut left = self.expr_add_sub_operand()?;
        let first_span = left.span();

        loop {
            let op = match self.peek(0) {
                Plus => Some(Binop::Add),
                Minus => Some(Binop::Sub),
                _ => None,
            };

            match op {
                Some(op) => {
                    self.next();
                    let right = self.expr_add_sub_operand()?;
                    left = Expr {
                        node_id: self.node(first_span),
                        kind: ExprKind::Binop {
                            left: Box::new(left),
                            op,
                            right: Box::new(right),
                        },
                    }
                }
                None => return Ok(left),
            }
        }
    }

    fn expr_add_sub_operand(&mut self) -> ParseResult<Expr> {
        let mut left = self.expr_mul_div_operand()?;
        let first_span = left.span();

        loop {
            let op = match self.peek(0) {
                Star => Some(Binop::Mul),
                Slash => Some(Binop::Div),
                _ => None,
            };

            match op {
                Some(op) => {
                    self.next();
                    let right = self.expr_mul_div_operand()?;
                    left = Expr {
                        node_id: self.node(first_span),
                        kind: ExprKind::Binop {
                            left: Box::new(left),
                            op,
                            right: Box::new(right),
                        },
                    }
                }
                None => return Ok(left),
            }
        }
    }

    fn expr_mul_div_operand(&mut self) -> ParseResult<Expr> {
        match self.peek(0) {
            Minus => {
                let first = self.next().unwrap();
                let right = self.expr_postfix()?;
                Ok(Expr {
                    node_id: self.node(first.span()),
                    kind: ExprKind::Unop {
                        op: Unop::Minus,
                        e: Box::new(right),
                    },
                })
            }
            _ => self.expr_postfix(),
        }
    }

    fn expr_postfix(&mut self) -> ParseResult<Expr> {
        let mut left = self.expr_primary()?;
        let first_span = left.span();
        loop {
            match self.peek(0) {
                Dot if self.peek(1) == Identifier => {
                    self.next();
                    let id = self.ident().unwrap();
                    left = Expr {
                        node_id: self.node(first_span),
                        kind: ExprKind::Dot {
                            e: Box::new(left),
                            id,
                        },
                    }
                }
                LeftSquare => {
                    let e2 = self.index()?;
                    left = Expr {
                        node_id: self.node(first_span),
                        kind: ExprKind::ArraySubscript {
                            e1: Box::new(left),
                            e2: Box::new(e2),
                        },
                    }
                }
                // Partial dot expression (mid-edits)
                // Accept the left-hand side and emit the parsing error
                Dot => {
                    self.next();

                    let Err(err) = self.ident() else {
                        unreachable!()
                    };

                    fpp_core::Diagnostic::from(err).emit();
                    return Ok(left);
                }
                _ => return Ok(left),
            }
        }
    }

    fn expr_primary(&mut self) -> ParseResult<Expr> {
        match self.peek(0) {
            LeftSquare => self.array_expr(),
            Keyword(False) => {
                let first = self.next().unwrap();
                Ok(Expr {
                    node_id: self.node(first.span()),
                    kind: ExprKind::LiteralBool(false),
                })
            }
            Keyword(True) => {
                let first = self.next().unwrap();
                Ok(Expr {
                    node_id: self.node(first.span()),
                    kind: ExprKind::LiteralBool(true),
                })
            }
            LiteralFloat => {
                let first = self.next().unwrap();
                Ok(Expr {
                    node_id: self.node(first.span()),
                    kind: ExprKind::LiteralFloat(first.text().to_string()),
                })
            }
            Identifier => {
                let first = self.next().unwrap();
                Ok(Expr {
                    node_id: self.node(first.span()),
                    kind: ExprKind::Ident(first.text().to_string()),
                })
            }
            LiteralInt => {
                let first = self.next().unwrap();
                Ok(Expr {
                    node_id: self.node(first.span()),
                    kind: ExprKind::LiteralInt(first.text().to_string()),
                })
            }
            LeftParen => {
                let first = self.next().unwrap();
                let e = self.expr()?;
                self.consume(RightParen)?;
                Ok(Expr {
                    node_id: self.node(first.span()),
                    kind: ExprKind::Paren(Box::new(e)),
                })
            }
            LiteralString | LiteralMultilineString { .. } => {
                let first = self.next().unwrap();
                Ok(Expr {
                    node_id: self.node(first.span()),
                    kind: ExprKind::LiteralString(first.text().to_string()),
                })
            }
            LeftCurly => self.struct_expr(),
            Keyword(Sizeof) => {
                let first = self.next().unwrap();
                self.consume(LeftParen)?;
                let type_name = self.type_name()?;
                self.consume(RightParen)?;
                Ok(Expr {
                    node_id: self.node(first.span()),
                    kind: ExprKind::SizeOf(Box::new(type_name)),
                })
            }
            _ => Err(self.cursor.err_expected_one_of(
                "expression expected",
                vec![
                    LeftSquare,
                    LeftCurly,
                    LeftParen,
                    Identifier,
                    Keyword(True),
                    Keyword(False),
                    Keyword(Sizeof),
                    LiteralFloat,
                    LiteralInt,
                    LiteralString,
                ],
            )),
        }
    }

    fn array_expr(&mut self) -> ParseResult<Expr> {
        let first = self.consume(LeftSquare)?;
        let members = self.element_sequence(&Parser::expr, Comma, RightSquare);
        self.consume(RightSquare)?;

        Ok(Expr {
            node_id: self.node(first.span()),
            kind: ExprKind::Array(members),
        })
    }

    fn struct_expr(&mut self) -> ParseResult<Expr> {
        let first = self.consume(LeftCurly)?;
        let members = self.element_sequence(&Parser::struct_expr_member, Comma, RightCurly);
        self.consume(RightCurly)?;

        Ok(Expr {
            node_id: self.node(first.span()),
            kind: ExprKind::Struct(members),
        })
    }

    fn struct_expr_member(&mut self) -> ParseResult<StructExprMember> {
        let name = self.name()?;
        self.consume(Equals)?;
        let value = self.expr()?;
        Ok(StructExprMember {
            node_id: self.node(name.span()),
            name,
            value,
        })
    }

    fn lit_string(&mut self) -> ParseResult<LitString> {
        let (tok, inner_span) = match self.peek(0) {
            LiteralString => {
                let token = self.next().unwrap();
                let inner_span = fpp_core::Span::new(
                    token.span.file(),
                    token.span.start().pos() + 1,
                    token.text().len() as BytePos,
                    token.span.including_span(),
                );

                (token, inner_span)
            }
            LiteralMultilineString { .. } => {
                let token = self.next().unwrap();
                let inner_span = fpp_core::Span::new(
                    token.span.file(),
                    token.span.start().pos() + 3,
                    token.text().len() as BytePos,
                    token.span.including_span(),
                );

                (token, inner_span)
            }
            _ => {
                return Err(self
                    .cursor
                    .err_expected_one_of("string literal expected", vec![LiteralString]));
            }
        };

        Ok(LitString {
            node_id: self.node(tok.span()),
            inner_span,
            data: tok.text().to_string(),
        })
    }

    fn formal_param_list(&mut self) -> ParseResult<FormalParamList> {
        match self.peek(0) {
            LeftParen => {
                self.next();
                let members =
                    self.annotated_element_sequence(&Parser::formal_param, Comma, RightParen);

                self.consume(RightParen)?;
                Ok(members)
            }
            _ => Ok(vec![]),
        }
    }

    fn formal_param(&mut self) -> ParseResult<FormalParam> {
        let first_span = self.current_span()?;

        let kind = match self.peek(0) {
            Keyword(Ref) => {
                self.next();
                FormalParamKind::Ref
            }
            _ => FormalParamKind::Value,
        };

        let name = self.name()?;
        self.consume(Colon)?;

        let type_name = self.type_name()?;

        Ok(FormalParam {
            node_id: self.node(first_span),
            kind,
            name,
            type_name,
        })
    }

    fn opt_expr(&mut self, prefix_keyword: KeywordKind) -> ParseResult<Option<Expr>> {
        if self.peek(0) == Keyword(prefix_keyword) {
            self.next();
            Ok(Some(self.expr()?))
        } else {
            Ok(None)
        }
    }

    fn current_span(&mut self) -> ParseResult<fpp_core::Span> {
        match self.peek_span(0) {
            Some(span) => Ok(span),
            None => Err(self.cursor.err_unexpected_eof()),
        }
    }

    /// Convenience function to consume a keyword
    #[inline]
    fn consume_keyword(&mut self, kind: KeywordKind) -> ParseResult<Token> {
        self.consume(Keyword(kind))
    }
}
