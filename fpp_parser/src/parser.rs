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
    /// The tokens that end the enclosing element sequences, innermost last.
    /// An element sequence with no enclosing token ends at the end of the input.
    ends: Vec<TokenKind>,
    /// The state of the element sequence element being parsed
    element: ElementState,
    /// An error at the end of the input that has propagated out of the element
    /// in which it occurred, to be reported by an enclosing element sequence
    eof_error: Option<ParseError>,
}

/// The state of the element being parsed in an element sequence.
///
/// The grammar is a set of backtracking parsers with commit points. A failure
/// before an element's commit point is not an error: the element sequence ends
/// before the element, and the error is reported by whatever follows the
/// sequence, usually as a missing closing token at the start of the element. A
/// failure after the commit point is reported where it occurs.
#[derive(Default)]
struct ElementState {
    /// Whether the element has passed a commit point
    committed: bool,
    /// The token at which the sequence ends if the element fails before its
    /// commit point, when a shorter alternative of the element parses up to
    /// there
    backtrack: Option<(fpp_core::Span, TokenKind)>,
    /// Whether an error at the end of the input inside the element survives
    /// the alternatives of the element, which it does when the element is the
    /// first alternative. Otherwise the alternatives before it fail at the
    /// start of the element, and that failure is reported instead.
    eof_error_survives: bool,
}

/// A type, array, constant, enum or struct definition: the definitions that
/// modules, components and state machines share, any of which may be a
/// dictionary definition
enum TypeOrConstantDef {
    AliasType(DefAliasType),
    AbsType(DefAbsType),
    Array(DefArray),
    Constant(DefConstant),
    Enum(DefEnum),
    Struct(DefStruct),
}

/// Convert a [TypeOrConstantDef] to a member of the member type `$member`
macro_rules! into_member {
    ($def:expr, $member:ident) => {
        match $def {
            TypeOrConstantDef::AliasType(def) => $member::DefAliasType(def),
            TypeOrConstantDef::AbsType(def) => $member::DefAbsType(def),
            TypeOrConstantDef::Array(def) => $member::DefArray(def),
            TypeOrConstantDef::Constant(def) => $member::DefConstant(def),
            TypeOrConstantDef::Enum(def) => $member::DefEnum(def),
            TypeOrConstantDef::Struct(def) => $member::DefStruct(def),
        }
    };
}

/// Parse a file with the entry point `entry`, which must consume all of it.
pub fn parse<T>(
    source_file: SourceFile,
    entry: fn(&mut Parser) -> T,
    include_span: Option<fpp_core::Span>,
) -> T {
    // We need our own copy of the source text since it needs to be long-lived
    let content = source_file.read().as_ref().to_string();
    let mut parser = Parser::new(source_file, content.as_ref(), include_span);

    let out = parser.parse_all_input(entry);
    parser.cursor.emit_errors();
    out
}

impl<'a> Parser<'a> {
    fn new(
        source_file: SourceFile,
        content: &'a str,
        include_span: Option<fpp_core::Span>,
    ) -> Parser<'a> {
        Parser {
            cursor: Cursor::new(source_file, content, include_span),
            include_span,
            ends: vec![],
            element: ElementState::default(),
            eof_error: None,
        }
    }

    /// Run `p` and require it to consume the whole input
    fn parse_all_input<T>(&mut self, p: fn(&mut Parser) -> T) -> T {
        let out = p(self);
        if let Some(err) = self.eof_error.take() {
            fpp_core::Diagnostic::from(err).emit();
        }
        if self.peek(0) != EOF {
            fpp_core::Diagnostic::from(self.unexpected_token()).emit();
        }
        out
    }

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
        if !is_dictionary_def {
            // `type` and a name alone are an abstract type definition
            self.element.backtrack = self.peek_token();
        }
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
        self.commit();
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
        self.commit();

        let name = self.name()?;

        self.consume(Equals)?;

        let size = self.index()?;
        let elt_type = self.type_name()?;

        let default = self.opt_expr(Default)?;

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
        self.commit();

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
        self.commit();
        self.consume_keyword(Component)?;
        let name = self.name()?;

        let members = self.enclosed(LeftCurly, RightCurly, Parser::component_members)?;

        Ok(DefComponent {
            node_id: self.node(first.span()),
            kind,
            name,
            members,
        })
    }

    pub fn trans_unit(&mut self) -> TransUnit {
        TransUnit(self.tu_members())
    }

    fn tu_members(&mut self) -> Vec<ModuleMember> {
        self.module_members()
    }

    pub fn component_members(&mut self) -> Vec<ComponentMember> {
        self.annotated_element_sequence(&Parser::component_member, Semi)
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
            Keyword(Dictionary | Type | Array | Constant | Enum | Struct) => {
                Ok(into_member!(self.type_or_constant_def()?, ComponentMember))
            }
            Keyword(State) => {
                if self.peek(2) == Keyword(Instance) {
                    Ok(ComponentMember::SpecStateMachineInstance(
                        self.spec_state_machine_instance()?,
                    ))
                } else {
                    Ok(ComponentMember::DefStateMachine(self.def_state_machine()?))
                }
            }
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
                    Keyword(Dictionary),
                    Keyword(Type),
                    Keyword(Array),
                    Keyword(Constant),
                    Keyword(Enum),
                    Keyword(State),
                    Keyword(Struct),
                    Keyword(Async),
                    Keyword(Sync),
                    Keyword(Guarded),
                    Keyword(Output),
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

    /// Parse a type, array, constant, enum or struct definition, any of which
    /// may be a dictionary definition. The next token is `dictionary`, `type`,
    /// `array`, `constant`, `enum` or `struct`.
    fn type_or_constant_def(&mut self) -> ParseResult<TypeOrConstantDef> {
        let kind = match self.peek(0) {
            Keyword(Dictionary) => self.peek(1),
            kind => kind,
        };
        match kind {
            Keyword(Type) if self.peek(0) == Keyword(Dictionary) || self.peek(2) == Equals => {
                Ok(TypeOrConstantDef::AliasType(self.alias_type()?))
            }
            Keyword(Type) => Ok(TypeOrConstantDef::AbsType(self.abs_type()?)),
            Keyword(Array) => Ok(TypeOrConstantDef::Array(self.def_array()?)),
            Keyword(Constant) => Ok(TypeOrConstantDef::Constant(self.def_constant()?)),
            Keyword(Enum) => Ok(TypeOrConstantDef::Enum(self.def_enum()?)),
            Keyword(Struct) => Ok(TypeOrConstantDef::Struct(self.def_struct()?)),
            _ => {
                self.consume_keyword(Dictionary)?;
                Err(self.cursor.err_expected_one_of(
                    "dictionary definition expected",
                    vec![
                        Keyword(Type),
                        Keyword(Array),
                        Keyword(Constant),
                        Keyword(Enum),
                        Keyword(Struct),
                    ],
                ))
            }
        }
    }

    fn def_module(&mut self) -> ParseResult<DefModule> {
        let first = self.consume_keyword(Module)?;
        self.commit();
        let name = self.name()?;
        let members = self.enclosed(LeftCurly, RightCurly, Parser::module_members)?;

        Ok(DefModule {
            node_id: self.node(first.span()),
            name,
            members,
        })
    }

    pub fn module_members(&mut self) -> Vec<ModuleMember> {
        self.annotated_element_sequence(&Parser::module_member, Semi)
    }

    fn module_member(&mut self) -> ParseResult<ModuleMember> {
        match self.peek(0) {
            Keyword(Dictionary | Type | Array | Constant | Enum | Struct) => {
                Ok(into_member!(self.type_or_constant_def()?, ModuleMember))
            }
            Keyword(Instance) => Ok(ModuleMember::DefComponentInstance(
                self.def_component_instance()?,
            )),
            Keyword(Passive) | Keyword(Active) | Keyword(Queued) => {
                Ok(ModuleMember::DefComponent(self.def_component()?))
            }
            Keyword(Interface) => Ok(ModuleMember::DefInterface(self.def_interface()?)),
            Keyword(Module) => match self.peek(1) {
                Keyword(Template) => {
                    Ok(ModuleMember::DefModuleTemplate(self.def_module_template()?))
                }
                _ => Ok(ModuleMember::DefModule(self.def_module()?)),
            },
            Keyword(Port) => Ok(ModuleMember::DefPort(self.def_port()?)),
            Keyword(State) => Ok(ModuleMember::DefStateMachine(self.def_state_machine()?)),
            Keyword(System) => Ok(ModuleMember::DefSystem(self.def_system()?)),
            Keyword(Deployment) | Keyword(Topology) => {
                Ok(ModuleMember::DefTopology(self.def_topology()?))
            }
            Keyword(Include) => Ok(ModuleMember::SpecInclude(self.spec_include()?)),
            Keyword(Locate) => Ok(ModuleMember::SpecLoc(self.spec_loc()?)),
            Keyword(Expand) => Ok(ModuleMember::SpecTemplateExpand(
                self.spec_template_expand()?,
            )),
            _ => Err(self.cursor.err_expected_one_of(
                "module member expected",
                vec![
                    Keyword(Dictionary),
                    Keyword(Type),
                    Keyword(Array),
                    Keyword(Constant),
                    Keyword(Enum),
                    Keyword(Struct),
                    Keyword(Active),
                    Keyword(Passive),
                    Keyword(Queued),
                    Keyword(Instance),
                    Keyword(Interface),
                    Keyword(Module),
                    Keyword(Port),
                    Keyword(State),
                    Keyword(System),
                    Keyword(Deployment),
                    Keyword(Topology),
                    Keyword(Include),
                    Keyword(Locate),
                    Keyword(Expand),
                ],
            )),
        }
    }

    fn def_module_template(&mut self) -> ParseResult<DefModuleTemplate> {
        let first = self.consume_keyword(Module)?;
        self.consume_keyword(Template)?;
        self.commit();
        let name = self.name()?;
        let params = self.template_param_list()?;
        let members = self.enclosed(LeftCurly, RightCurly, Parser::module_members)?;

        Ok(DefModuleTemplate {
            node_id: self.node(first.span()),
            name,
            params,
            members,
        })
    }

    fn template_param_list(&mut self) -> ParseResult<TemplateParamList> {
        match self.peek(0) {
            LeftParen => self.enclosed(LeftParen, RightParen, |p| {
                p.annotated_element_sequence(&Parser::template_param, Comma)
            }),
            _ => Ok(vec![]),
        }
    }

    fn template_param(&mut self) -> ParseResult<TemplateParam> {
        let first_span = self.current_span()?;

        match self.peek(0) {
            Keyword(Constant) => {
                self.first_alternative();
                self.next();
                self.commit();
                let name = self.name()?;
                self.consume(Colon)?;
                let type_name = self.type_name()?;
                Ok(TemplateParam::Constant(TemplateParamConstant {
                    node_id: self.node(first_span),
                    name,
                    type_name,
                }))
            }
            Keyword(Type) => {
                self.next();
                self.commit();
                let name = self.name()?;
                Ok(TemplateParam::Type(TemplateParamType {
                    node_id: self.node(first_span),
                    name,
                }))
            }
            Keyword(Instance) => {
                self.next();
                self.commit();
                let name = self.name()?;
                self.consume(Colon)?;
                let interface = self.qual_ident()?;
                Ok(TemplateParam::Interface(TemplateParamInterface {
                    node_id: self.node(first_span),
                    name,
                    interface,
                }))
            }
            _ => Err(self.cursor.err_expected_one_of(
                "template parameter expected",
                vec![Keyword(Constant), Keyword(Type), Keyword(Instance)],
            )),
        }
    }

    fn spec_template_expand(&mut self) -> ParseResult<SpecTemplateExpand> {
        let first = self.consume_keyword(Expand)?;
        self.commit();
        let template = self.qual_ident()?;
        let args = self.template_arg_list()?;

        Ok(SpecTemplateExpand {
            node_id: self.node(first.span()),
            template,
            args,
            members: None,
        })
    }

    fn template_arg_list(&mut self) -> ParseResult<TemplateArgList> {
        match self.peek(0) {
            LeftParen => self.enclosed(LeftParen, RightParen, |p| {
                p.element_sequence(&Parser::template_arg, Comma)
            }),
            _ => Ok(vec![]),
        }
    }

    fn template_arg(&mut self) -> ParseResult<TemplateArg> {
        let first_span = self.current_span()?;

        let kind = match self.peek(0) {
            Keyword(Constant) => {
                self.first_alternative();
                self.next();
                self.commit();
                TemplateArgKind::Constant(self.expr()?)
            }
            Keyword(Type) => {
                self.next();
                self.commit();
                TemplateArgKind::Type(self.type_name()?)
            }
            Keyword(Instance) => {
                self.next();
                self.commit();
                TemplateArgKind::Interface(self.qual_ident()?)
            }
            _ => {
                return Err(self.cursor.err_expected_one_of(
                    "template argument expected",
                    vec![Keyword(Constant), Keyword(Type), Keyword(Instance)],
                ));
            }
        };

        Ok(TemplateArg {
            node_id: self.node(first_span),
            kind,
        })
    }

    fn def_port(&mut self) -> ParseResult<DefPort> {
        let first = self.consume_keyword(Port)?;
        self.commit();
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
        self.commit();

        // Only a constant or a type may be a dictionary definition
        let (is_dictionary_def, kind) = match self.peek(0) {
            Keyword(Dictionary) => {
                self.next();
                match self.peek(0) {
                    Keyword(Constant) => {
                        self.next();
                        (true, SpecLocKind::Constant)
                    }
                    Keyword(Type) => {
                        self.next();
                        (true, SpecLocKind::Type)
                    }
                    _ => {
                        return Err(self.cursor.err_expected_one_of(
                            "dictionary location kind expected",
                            vec![Keyword(Constant), Keyword(Type)],
                        ));
                    }
                }
            }
            Keyword(Constant) => {
                self.next();
                (false, SpecLocKind::Constant)
            }
            Keyword(Type) => {
                self.next();
                (false, SpecLocKind::Type)
            }
            Keyword(Component) => {
                self.next();
                (false, SpecLocKind::Component)
            }
            Keyword(Instance) => {
                self.next();
                (false, SpecLocKind::Instance)
            }
            Keyword(Port) => {
                self.next();
                (false, SpecLocKind::Port)
            }
            Keyword(State) => {
                self.next();
                self.consume_keyword(Machine)?;
                (false, SpecLocKind::StateMachine)
            }
            Keyword(Template) => {
                self.next();
                (false, SpecLocKind::Template)
            }
            Keyword(System) => {
                self.next();
                (false, SpecLocKind::System)
            }
            Keyword(Interface) => {
                self.next();
                (false, SpecLocKind::Interface)
            }
            _ => {
                return Err(self.cursor.err_expected_one_of(
                    "dictionary specifier or location kind expected",
                    vec![
                        Keyword(Dictionary),
                        Keyword(Constant),
                        Keyword(Type),
                        Keyword(Component),
                        Keyword(Instance),
                        Keyword(Port),
                        Keyword(State),
                        Keyword(Template),
                        Keyword(System),
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
        self.commit();
        let name = self.name()?;
        let implements = match self.peek(0) {
            Keyword(Implements) => {
                self.next();
                self.until(LeftCurly, |p| {
                    p.element_sequence(
                        &|p| {
                            p.first_alternative();
                            p.qual_ident()
                        },
                        Comma,
                    )
                })
            }
            _ => vec![],
        };

        let members = self.enclosed(LeftCurly, RightCurly, Parser::topology_members)?;

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
        self.commit();
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
        self.annotated_element_sequence(&Parser::topology_member, Semi)
    }

    fn topology_member(&mut self) -> ParseResult<TopologyMember> {
        match self.peek(0) {
            Keyword(Import) | Keyword(Instance) => {
                self.first_alternative();
                Ok(TopologyMember::SpecInstance(self.spec_instance()?))
            }
            Keyword(Include) => Ok(TopologyMember::SpecInclude(self.spec_include()?)),
            Keyword(Port) => Ok(TopologyMember::SpecTopPort(self.spec_top_port()?)),
            Keyword(Telemetry) if self.peek(1) == Keyword(Packets) => Ok(
                TopologyMember::SpecTlmPacketSet(self.spec_tlm_packet_set()?),
            ),
            Keyword(Connections | Command | Event | Health | Param | Telemetry | Text | Time) => {
                self.spec_connection_graph()
            }
            _ => Err(self.cursor.err_expected_one_of(
                "topology member expected",
                vec![
                    Keyword(Instance),
                    Keyword(Import),
                    Keyword(Connections),
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

    /// Parse a direct or pattern connection graph specifier
    fn spec_connection_graph(&mut self) -> ParseResult<TopologyMember> {
        match self.peek(0) {
            Keyword(Connections) => Ok(TopologyMember::SpecDirectConnectionGraph(
                self.spec_connection_graph_direct()?,
            )),
            _ => Ok(TopologyMember::SpecPatternConnectionGraph(
                self.spec_connection_graph_pattern()?,
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
        self.commit();
        self.consume_keyword(Instance)?;
        let source = self.qual_ident()?;

        let targets = match self.peek(0) {
            LeftCurly => self.enclosed(LeftCurly, RightCurly, |p| {
                p.element_sequence(
                    &|p| {
                        p.first_alternative();
                        p.qual_ident()
                    },
                    Comma,
                )
            })?,
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
        self.commit();
        let connections = self.enclosed(LeftCurly, RightCurly, |p| {
            p.element_sequence(&Parser::connection, Comma)
        })?;

        Ok(SpecDirectConnectionGraph {
            node_id: self.node(first.span()),
            name,
            connections,
        })
    }

    fn connection(&mut self) -> ParseResult<Connection> {
        self.first_alternative();
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
        self.commit();

        let instance = self.qual_ident()?;
        Ok(SpecInstance {
            node_id: self.node(first.span()),
            instance,
        })
    }

    fn spec_tlm_packet_set(&mut self) -> ParseResult<SpecTlmPacketSet> {
        let first = self.consume_keyword(Telemetry)?;
        self.consume_keyword(Packets)?;
        self.commit();
        let name = self.name()?;
        let members = self.enclosed(LeftCurly, RightCurly, Parser::tlm_packet_set_members)?;

        let omitted = match self.peek(0) {
            Keyword(Omit) => {
                self.next();
                self.enclosed(LeftCurly, RightCurly, |p| {
                    p.element_sequence(
                        &|p| {
                            p.first_alternative();
                            p.tlm_channel_identifier()
                        },
                        Comma,
                    )
                })?
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
        self.annotated_element_sequence(&Parser::tlm_packet_set_member, Comma)
    }

    fn tlm_packet_set_member(&mut self) -> ParseResult<TlmPacketSetMember> {
        match self.peek(0) {
            Keyword(Include) => {
                self.first_alternative();
                Ok(TlmPacketSetMember::SpecInclude(self.spec_include()?))
            }
            Keyword(Packet) => Ok(TlmPacketSetMember::SpecTlmPacket(self.spec_tlm_packet()?)),
            _ => Err(self.cursor.err_expected_one_of(
                "telemetry packet set member expected",
                vec![Keyword(Include), Keyword(Packet)],
            )),
        }
    }

    fn spec_tlm_packet(&mut self) -> ParseResult<SpecTlmPacket> {
        let first = self.consume_keyword(Packet)?;
        self.commit();
        let name = self.name()?;
        let id = self.opt_expr(Id)?;
        self.consume_keyword(Group)?;
        let group = self.expr()?;
        let members = self.enclosed(LeftCurly, RightCurly, Parser::tlm_packet_members)?;

        Ok(SpecTlmPacket {
            node_id: self.node(first.span()),
            name,
            id,
            group,
            members,
        })
    }

    pub fn tlm_packet_members(&mut self) -> Vec<TlmPacketMember> {
        self.element_sequence(&Parser::tlm_packet_member, Comma)
    }

    fn tlm_packet_member(&mut self) -> ParseResult<TlmPacketMember> {
        match self.peek(0) {
            Keyword(Include) => {
                self.first_alternative();
                Ok(TlmPacketMember::SpecInclude(self.spec_include()?))
            }
            Identifier | Dot => Ok(TlmPacketMember::TlmChannelIdentifier(
                self.tlm_channel_identifier()?,
            )),
            _ => Err(self.cursor.err_expected_one_of(
                "telemetry packet member expected",
                vec![Keyword(Include), Identifier, Dot],
            )),
        }
    }

    fn tlm_channel_identifier(&mut self) -> ParseResult<TlmChannelIdentifier> {
        let dot = self.opt_dot();
        let id = self.ident()?;
        self.commit();
        self.consume(Dot)?;
        let mut qid = self.qual_ident_node_list()?;
        let channel_name = qid.pop().unwrap();
        let mut component_instance = vec![id];
        component_instance.extend(qid);
        let first_span = dot.unwrap_or(component_instance[0].span());
        let component_instance = QualIdent::from_node_list(component_instance, dot.is_some());

        Ok(TlmChannelIdentifier {
            node_id: self.node(first_span),
            component_instance,
            channel_name,
        })
    }

    fn port_instance_identifier(&mut self) -> ParseResult<PortInstanceIdentifier> {
        let dot = self.opt_dot();
        let id = self.ident()?;
        self.commit();
        self.consume(Dot)?;
        let mut qid = self.qual_ident_node_list()?;
        let port_name = qid.pop().unwrap();
        let mut component_instance = vec![id];
        component_instance.extend(qid);
        let first_span = dot.unwrap_or(component_instance[0].span());
        let interface_instance = QualIdent::from_node_list(component_instance, dot.is_some());

        Ok(PortInstanceIdentifier {
            node_id: self.node(first_span),
            interface_instance,
            port_name,
        })
    }

    /// An optional `.`, returning its span if it is there
    fn opt_dot(&mut self) -> Option<fpp_core::Span> {
        match self.peek(0) {
            Dot => self.next().map(|dot| dot.span()),
            _ => None,
        }
    }

    fn spec_top_port(&mut self) -> ParseResult<SpecTopPort> {
        let first = self.consume_keyword(Port)?;
        self.commit();
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
        self.commit();
        let name = self.name()?;
        self.consume(Colon)?;
        let component = self.qual_ident()?;

        self.consume_keyword(Base)?;
        self.consume_keyword(Id)?;
        let base_id = self.expr()?;

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
                LeftCurly => self.enclosed(LeftCurly, RightCurly, |p| {
                    p.annotated_element_sequence(&Parser::spec_init, Semi)
                })?,
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
        self.first_alternative();
        let first = self.consume_keyword(Phase)?;
        self.commit();
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
        self.commit();
        let name = self.name()?;
        let members = self.enclosed(LeftCurly, RightCurly, Parser::interface_members)?;

        Ok(DefInterface {
            node_id: self.node(first.span()),
            name,
            members,
        })
    }

    fn interface_members(&mut self) -> Vec<InterfaceMember> {
        self.annotated_element_sequence(&Parser::interface_member, Semi)
    }

    fn interface_member(&mut self) -> ParseResult<InterfaceMember> {
        match self.peek(0) {
            Keyword(Import) => Ok(InterfaceMember::SpecInterfaceImport(
                self.spec_import_interface()?,
            )),
            Keyword(
                Async | Guarded | Sync | Output | Command | Event | Param | Product | Telemetry
                | Text | Time,
            ) => {
                if self.peek(0) == Keyword(Output) || self.peek(1) == Keyword(Input) {
                    // A general port instance is the first alternative of a
                    // member
                    self.first_alternative();
                }
                Ok(InterfaceMember::SpecPortInstance(
                    self.spec_port_instance()?,
                ))
            }
            _ => Err(self.cursor.err_expected_one_of(
                "interface member expected",
                vec![
                    Keyword(Async),
                    Keyword(Guarded),
                    Keyword(Sync),
                    Keyword(Output),
                    Keyword(Command),
                    Keyword(Event),
                    Keyword(Param),
                    Keyword(Product),
                    Keyword(Telemetry),
                    Keyword(Text),
                    Keyword(Time),
                    Keyword(Import),
                ],
            )),
        }
    }

    fn def_constant(&mut self) -> ParseResult<DefConstant> {
        let (first, is_dictionary_def) = self.dictionary_opt(Constant)?;
        self.commit();
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
        self.commit();
        let name = self.name()?;

        let type_name = match self.peek(0) {
            Colon => {
                self.next();
                Some(self.type_name()?)
            }
            _ => None,
        };

        let constants = self.enclosed(LeftCurly, RightCurly, |p| {
            p.annotated_element_sequence(&Parser::def_enum_constant, Comma)
        })?;

        let default = self.opt_expr(Default)?;

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
        self.first_alternative();
        let name = self.name()?;
        self.commit();
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
        self.commit();

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
        self.commit();
        let members = match self.peek(0) {
            LeftCurly => {
                Some(self.enclosed(LeftCurly, RightCurly, Parser::state_machine_members)?)
            }
            _ => None,
        };

        Ok(DefStateMachine {
            node_id: self.node(first.span()),
            name,
            members,
        })
    }

    pub fn state_machine_members(&mut self) -> Vec<StateMachineMember> {
        self.annotated_element_sequence(&Parser::state_machine_member, Semi)
    }

    fn state_machine_member(&mut self) -> ParseResult<StateMachineMember> {
        match self.peek(0) {
            Keyword(Dictionary | Type | Array | Constant | Enum | Struct) => Ok(into_member!(
                self.type_or_constant_def()?,
                StateMachineMember
            )),
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
                    Keyword(Dictionary),
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

    pub fn state_members(&mut self) -> Vec<StateMember> {
        self.annotated_element_sequence(&Parser::state_member, Semi)
    }

    fn state_member(&mut self) -> ParseResult<StateMember> {
        match self.peek(0) {
            Keyword(Choice) => {
                self.first_alternative();
                Ok(StateMember::DefChoice(self.def_choice()?))
            }
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
        self.commit();
        let actions = self.enclosed(LeftCurly, RightCurly, |p| {
            p.element_sequence(
                &|p| {
                    p.first_alternative();
                    p.ident()
                },
                Comma,
            )
        })?;

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
        self.commit();
        let guard = match (self.peek(0), self.peek(1)) {
            (Keyword(If), Identifier) => {
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

        // After a do expression, the transition alternative needs `enter` and
        // a target; otherwise the do expression alone is the alternative
        let is_transition = self.peek(0) == Keyword(Enter)
            && (do_expr.is_none()
                || self.peek(1) == Identifier
                || (self.peek(1) == Dot && self.peek(2) == Identifier));

        match (is_transition, do_expr) {
            (true, actions) => {
                self.next();
                let target = self.qual_ident()?;
                Ok(TransitionOrDo::Transition(TransitionExpr {
                    node_id: self.node(first_span),
                    actions,
                    target,
                }))
            }
            (false, Some(de)) => Ok(TransitionOrDo::Do(de)),
            (false, None) => Err(self.cursor.err_expected_one_of(
                "expected transition or do",
                vec![Keyword(Do), Keyword(Enter)],
            )),
        }
    }

    fn transition_expr(&mut self) -> ParseResult<TransitionExpr> {
        let first_span = self.current_span()?;

        let do_expr = match self.peek(0) {
            Keyword(Do) => Some(self.scoped(|p| p.do_expr())?),
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
                match do_expr {
                    Some(_) => vec![Keyword(Enter)],
                    None => vec![Keyword(Enter), Keyword(Do)],
                },
            )),
        }
    }

    fn def_guard(&mut self) -> ParseResult<DefGuard> {
        let first = self.consume_keyword(Guard)?;
        let name = self.name()?;
        self.commit();
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
        self.commit();
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
        self.commit();
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
        self.commit();
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
        self.commit();
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
        self.commit();
        let file = self.lit_string()?;
        Ok(SpecInclude {
            node_id: self.node(first.span()),
            file,
        })
    }

    fn spec_import_interface(&mut self) -> ParseResult<SpecInterfaceImport> {
        let first = self.consume_keyword(Import)?;
        self.commit();
        let interface = self.qual_ident()?;
        Ok(SpecInterfaceImport {
            node_id: self.node(first.span()),
            interface,
        })
    }

    fn spec_internal_port(&mut self) -> ParseResult<SpecInternalPort> {
        let first = self.consume_keyword(Internal)?;
        self.commit();
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
        self.commit();
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
        // The name is committed, but the `:` after it is not
        let name = self.scoped(|p| {
            p.commit();
            p.name()
        })?;
        self.consume(Colon)?;
        self.commit();
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
        self.commit();
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
        self.enclosed(LeftCurly, RightCurly, |p| {
            p.element_sequence(&Parser::limit, Comma)
        })
    }

    fn limit(&mut self) -> ParseResult<TlmChannelLimit> {
        self.first_alternative();
        let first_span = self.current_span()?;
        let kind = self.limit_kind()?;
        self.commit();
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
        self.commit();
        let name = self.name()?;

        let members = self.enclosed(LeftCurly, RightCurly, |p| {
            p.annotated_element_sequence(&Parser::struct_type_member, Comma)
        })?;

        let default = self.opt_expr(Default)?;

        Ok(DefStruct {
            node_id: self.node(first.span()),
            name,
            members,
            default,
            is_dictionary_def,
        })
    }

    fn struct_type_member(&mut self) -> ParseResult<StructTypeMember> {
        self.first_alternative();
        let name = self.name()?;
        self.commit();
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
        self.commit();

        let members = match self.peek(0) {
            LeftCurly => self.enclosed(LeftCurly, RightCurly, Parser::state_members)?,
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
        self.commit();
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
        self.commit();

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
            Identifier | Dot => Some(self.qual_ident()?),
            _ => {
                return Err(self.cursor.err_expected_one_of(
                    "port type expected",
                    vec![Identifier, Dot, Keyword(Serial)],
                ));
            }
        };

        let priority = self.opt_expr(Priority)?;

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
            Keyword(Command) => {
                self.next();
                match self.peek(0) {
                    Keyword(Recv) => {
                        self.next();
                        Ok(SpecialPortInstanceKind::CommandRecv)
                    }
                    Keyword(Reg) => {
                        self.next();
                        Ok(SpecialPortInstanceKind::CommandReg)
                    }
                    Keyword(Resp) => {
                        self.next();
                        Ok(SpecialPortInstanceKind::CommandResp)
                    }
                    _ => Err(self.cursor.err_expected_one_of(
                        "command special port expected",
                        vec![Keyword(Recv), Keyword(Reg), Keyword(Resp)],
                    )),
                }
            }
            Keyword(Event) => {
                self.next();
                Ok(SpecialPortInstanceKind::Event)
            }
            Keyword(Param) => {
                self.next();
                match self.peek(0) {
                    Keyword(Get) => {
                        self.next();
                        Ok(SpecialPortInstanceKind::ParamGet)
                    }
                    Keyword(Set) => {
                        self.next();
                        Ok(SpecialPortInstanceKind::ParamSet)
                    }
                    _ => Err(self.cursor.err_expected_one_of(
                        "param special port expected",
                        vec![Keyword(Get), Keyword(Set)],
                    )),
                }
            }
            Keyword(Product) => {
                self.next();
                match self.peek(0) {
                    Keyword(Get) => {
                        self.next();
                        Ok(SpecialPortInstanceKind::ProductGet)
                    }
                    Keyword(Recv) => {
                        self.next();
                        Ok(SpecialPortInstanceKind::ProductRecv)
                    }
                    Keyword(Request) => {
                        self.next();
                        Ok(SpecialPortInstanceKind::ProductRequest)
                    }
                    Keyword(Send) => {
                        self.next();
                        Ok(SpecialPortInstanceKind::ProductSend)
                    }
                    _ => Err(self.cursor.err_expected_one_of(
                        "product special port expected",
                        vec![Keyword(Get), Keyword(Recv), Keyword(Request), Keyword(Send)],
                    )),
                }
            }
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
        self.commit();
        let name = self.name()?;

        let priority = self.opt_expr(Priority)?;

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
        self.commit();
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

    /// The token that ends the innermost element sequence
    fn sequence_end(&self) -> TokenKind {
        self.ends.last().copied().unwrap_or(EOF)
    }

    /// Run `p` with `end` as the token that ends its element sequences
    fn until<T>(&mut self, end: TokenKind, p: impl FnOnce(&mut Self) -> T) -> T {
        self.ends.push(end);
        let out = p(self);
        self.ends.pop();
        out
    }

    /// Parse `open`, then `p` with `close` as the token that ends its element
    /// sequences, then `close`
    fn enclosed<T>(
        &mut self,
        open: TokenKind,
        close: TokenKind,
        p: impl FnOnce(&mut Self) -> T,
    ) -> ParseResult<T> {
        self.consume(open)?;
        let out = self.until(close, p);
        self.consume(close)?;
        Ok(out)
    }

    /// Mark a commit point of the element being parsed
    fn commit(&mut self) {
        self.element.committed = true;
    }

    /// Run `p`, which may have commit points of its own. If `p` succeeds, the
    /// element is committed only as far as it was before `p`.
    fn scoped<T>(&mut self, p: impl FnOnce(&mut Self) -> ParseResult<T>) -> ParseResult<T> {
        let committed = self.element.committed;
        let out = p(self);
        if out.is_ok() {
            self.element.committed = committed;
        }
        out
    }

    /// Mark the element being parsed as the first alternative of its sequence
    fn first_alternative(&mut self) {
        self.element.eof_error_survives = true;
    }

    /// The span and kind of the next token, if any
    fn peek_token(&mut self) -> Option<(fpp_core::Span, TokenKind)> {
        let span = self.peek_span(0)?;
        Some((span, self.peek(0)))
    }

    /// The error for an unexpected next token
    fn unexpected_token(&mut self) -> ParseError {
        match self.peek_token() {
            Some((got_span, got_kind)) => ParseError::UnexpectedToken { got_kind, got_span },
            None => self.cursor.err_unexpected_eof(),
        }
    }

    /// The error for an element sequence that ends at the token `got`: what
    /// follows the sequence is expected there instead
    fn sequence_ended_at(&self, (span, got): (fpp_core::Span, TokenKind)) -> ParseError {
        match self.sequence_end() {
            EOF => ParseError::UnexpectedToken {
                got_kind: got,
                got_span: span,
            },
            end => ParseError::ExpectedToken {
                expected: end,
                got,
                last: span,
                msg: "unexpected token",
            },
        }
    }

    /// The error for an element sequence that ends at the next token
    fn sequence_ended_at_next(&mut self) -> ParseError {
        match self.peek_token() {
            Some(token) => self.sequence_ended_at(token),
            None => self.cursor.err_unexpected_eof(),
        }
    }

    /// Parse an element of a sequence with `elt`. On failure, return the
    /// error together with the state of the element at the failure.
    fn element<T>(
        &mut self,
        elt: &dyn Fn(&mut Parser<'a>) -> ParseResult<T>,
    ) -> Result<T, (ParseError, ElementState)> {
        let outer = std::mem::take(&mut self.element);
        let out = elt(self);
        let inner = std::mem::replace(&mut self.element, outer);
        out.map_err(|err| (err, inner))
    }

    /// Report the failure of an element of a sequence, and skip what is left
    /// of it. `annotation_start` is the first pre-annotation of the element,
    /// if it has any, and `element_start` is the token after the
    /// pre-annotations, if there is one. Returns whether the sequence goes on.
    fn element_failed(
        &mut self,
        err: ParseError,
        state: ElementState,
        annotation_start: Option<(fpp_core::Span, TokenKind)>,
        element_start: Option<(fpp_core::Span, TokenKind)>,
        punct: TokenKind,
    ) -> bool {
        let at_eof = self.peek(0) == EOF;
        let file_level = self.sequence_end() == EOF;
        // An error at the end of the input is the lowest-placed failure, so
        // unless the element is the first alternative, the failure of an
        // earlier alternative at the start of the element replaces it
        let dropped = state.committed && at_eof && !state.eof_error_survives;
        let err = if state.committed && !dropped {
            if !at_eof {
                err
            } else if file_level {
                self.eof_error.take().unwrap_or(err)
            } else {
                // The error propagates to the enclosing sequence
                self.eof_error.get_or_insert(err);
                return false;
            }
        } else if let Some(stop) = state.backtrack {
            // The shorter alternative of the element is the element, and the
            // sequence ends after it
            self.sequence_ended_at_stop(err, stop, dropped)
        } else if let (Some(_), Some(element_start)) = (annotation_start, element_start) {
            // The pre-annotations commit to the element, which fails where it
            // stops, or at its start if it stops at the end of the input
            if at_eof {
                ParseError::UnexpectedToken {
                    got_kind: element_start.1,
                    got_span: element_start.0,
                }
            } else {
                err
            }
        } else if let Some(annotation_start) = annotation_start {
            // Pre-annotations at the end of the input
            if file_level {
                self.eof_error.take().unwrap_or(err)
            } else {
                self.sequence_ended_at_stop(err, annotation_start, dropped)
            }
        } else {
            // The sequence ends before the element
            let stop = element_start.expect("an element starts at a token");
            if self.peek_span(0) == Some(stop.0) {
                // Nothing was consumed, so the element's own error says what
                // is expected at its start
                err
            } else {
                self.sequence_ended_at_stop(err, stop, dropped)
            }
        };

        fpp_core::Diagnostic::from(err).emit();
        self.recover(punct)
    }

    /// The error for an element sequence that ends at the token `stop`
    /// because its element `err` failed. At the top level, the first error
    /// that passed a commit point is the error, if there is one.
    fn sequence_ended_at_stop(
        &mut self,
        err: ParseError,
        stop: (fpp_core::Span, TokenKind),
        committed: bool,
    ) -> ParseError {
        if self.sequence_end() == EOF {
            match self.eof_error.take() {
                Some(eof_error) => eof_error,
                None if committed => err,
                None => self.sequence_ended_at(stop),
            }
        } else {
            self.eof_error = None;
            self.sequence_ended_at(stop)
        }
    }

    /// Skip the rest of a failed element: past the next separator or end of
    /// line outside any enclosed tokens, or up to a token that ends an
    /// enclosing element sequence. Returns whether the sequence goes on.
    fn recover(&mut self, punct: TokenKind) -> bool {
        let mut depth = 0usize;
        loop {
            let current = self.peek(0);
            if current == EOF {
                return false;
            }
            if depth == 0 {
                if current == punct || current == Eol {
                    self.next();
                    return true;
                }
                if self.ends.contains(&current) {
                    return false;
                }
            }
            match current {
                LeftCurly | LeftParen | LeftSquare => depth += 1,
                RightCurly | RightParen | RightSquare => depth = depth.saturating_sub(1),
                _ => {}
            }
            self.next();
        }
    }

    /// Parse a sequence of elements with optional annotations, each terminated
    /// by `punct` or an end of line, except that the last one need not be.
    /// The sequence may start with ends of line.
    fn annotated_element_sequence<T: fpp_core::Annotated + AstNode>(
        &mut self,
        elt: &dyn Fn(&mut Parser<'a>) -> ParseResult<T>,
        punct: TokenKind,
    ) -> Vec<T> {
        let end = self.sequence_end();
        while self.peek(0) == Eol {
            self.next();
        }

        let mut out: Vec<T> = vec![];
        loop {
            let annotation_start = match self.peek(0) {
                PreAnnotation => self.peek_token(),
                _ => None,
            };
            let pre_annotation = self.pre_annotation();
            let element_start = self.peek_token();
            let next = self.peek(0);
            if annotation_start.is_none() && (next == end || next == EOF) {
                break;
            }

            match self.element(elt) {
                Ok(data) => {
                    let terminator = self.peek(0);
                    if terminator == punct || terminator == Eol {
                        self.next();
                    }
                    let post_annotation = self.post_annotation();
                    let terminated =
                        terminator == punct || terminator == Eol || !post_annotation.is_empty();
                    fpp_core::Node::annotate(&data.id(), pre_annotation, post_annotation);
                    out.push(data);

                    if !terminated {
                        // An unterminated element ends the sequence
                        if terminator == end || terminator == EOF {
                            break;
                        }
                        fpp_core::Diagnostic::from(self.sequence_ended_at_next()).emit();
                        if !self.recover(punct) {
                            break;
                        }
                    }
                }
                Err((err, state)) => {
                    if !self.element_failed(err, state, annotation_start, element_start, punct) {
                        break;
                    }
                }
            }
        }

        out
    }

    /// Parse a sequence of elements separated by `sep` or an end of line. A
    /// separator may follow the last element, or stand alone in an empty
    /// sequence.
    fn element_sequence<T>(
        &mut self,
        elt: &dyn Fn(&mut Parser<'a>) -> ParseResult<T>,
        sep: TokenKind,
    ) -> Vec<T> {
        let end = self.sequence_end();
        let mut out: Vec<T> = vec![];
        let mut at_start = true;
        loop {
            let next = self.peek(0);
            if next == end || next == EOF {
                break;
            }
            if next == sep || next == Eol {
                if at_start {
                    self.next();
                    break;
                }
                // A separator where an element should be ends the sequence
                // at the separator before it
                fpp_core::Diagnostic::from(self.sequence_ended_at_next()).emit();
                if !self.recover(sep) {
                    break;
                }
                continue;
            }

            at_start = false;
            let start = self.peek_token();
            match self.element(elt) {
                Ok(data) => {
                    out.push(data);
                    let next = self.peek(0);
                    if next == sep || next == Eol {
                        self.next();
                    } else if next == end || next == EOF {
                        break;
                    } else {
                        // A missing separator ends the sequence
                        fpp_core::Diagnostic::from(self.sequence_ended_at_next()).emit();
                        if !self.recover(sep) {
                            break;
                        }
                    }
                }
                Err((err, state)) => {
                    if !self.element_failed(err, state, None, start, sep) {
                        break;
                    }
                }
            }
        }

        out
    }

    fn index(&mut self) -> ParseResult<Expr> {
        self.consume(LeftSquare)?;
        self.commit();
        let out = self.expr()?;
        self.consume(RightSquare)?;
        Ok(out)
    }

    fn qual_ident(&mut self) -> ParseResult<QualIdent> {
        let dot = self.opt_dot();
        let qid = self.qual_ident_node_list()?;
        Ok(QualIdent::from_node_list(qid, dot.is_some()))
    }

    fn qual_ident_node_list(&mut self) -> ParseResult<QualIdentNodeList> {
        let mut out = vec![self.ident()?];
        while self.peek(0) == Dot && self.peek(1) == Identifier {
            self.next();
            out.push(self.ident()?)
        }

        Ok(out)
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
                let size = self.opt_expr(Size)?;
                Ok(TypeNameKind::String(size))
            }
            Identifier | Dot => Ok(TypeNameKind::QualIdent(self.qual_ident()?)),
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
                    Dot,
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
                    self.commit();
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
                    self.commit();
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
                    self.commit();
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
                self.commit();
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
                    let bracket = self.peek_token().unwrap();
                    self.next();
                    let committed = std::mem::replace(&mut self.element.committed, false);
                    let e2 = match self.expr() {
                        Ok(e2) => e2,
                        Err(_) if !self.element.committed => {
                            // No subscript follows, so the expression ends
                            // before the `[`
                            self.element.committed = true;
                            return Err(self.sequence_ended_at(bracket));
                        }
                        Err(err) => return Err(err),
                    };
                    self.element.committed = committed;
                    self.commit();
                    self.consume(RightSquare)?;
                    left = Expr {
                        node_id: self.node(first_span),
                        kind: ExprKind::ArraySubscript {
                            e1: Box::new(left),
                            e2: Box::new(e2),
                        },
                    }
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
                    kind: ExprKind::Ident {
                        value: first.text().to_string(),
                        is_absolute: false,
                    },
                })
            }
            Dot => {
                let first = self.next().unwrap();
                let id = self.consume(Identifier)?;
                Ok(Expr {
                    node_id: self.node(first.span()),
                    kind: ExprKind::Ident {
                        value: id.text().to_string(),
                        is_absolute: true,
                    },
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
                let e = self.scoped(|p| p.expr())?;
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
                let type_name = self.scoped(|p| p.type_name())?;
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
                    Dot,
                    Minus,
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
        let first = self.current_span()?;
        let members = self.enclosed(LeftSquare, RightSquare, |p| {
            p.commit();
            p.element_sequence(
                &|p| {
                    p.first_alternative();
                    p.expr()
                },
                Comma,
            )
        })?;

        Ok(Expr {
            node_id: self.node(first),
            kind: ExprKind::Array(members),
        })
    }

    fn struct_expr(&mut self) -> ParseResult<Expr> {
        let first = self.current_span()?;
        let members = self.enclosed(LeftCurly, RightCurly, |p| {
            p.commit();
            p.element_sequence(&Parser::struct_expr_member, Comma)
        })?;

        Ok(Expr {
            node_id: self.node(first),
            kind: ExprKind::Struct(members),
        })
    }

    fn struct_expr_member(&mut self) -> ParseResult<StructExprMember> {
        self.first_alternative();
        let name = self.name()?;
        self.commit();
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
            LeftParen => self.enclosed(LeftParen, RightParen, |p| {
                p.annotated_element_sequence(&Parser::formal_param, Comma)
            }),
            _ => Ok(vec![]),
        }
    }

    fn formal_param(&mut self) -> ParseResult<FormalParam> {
        self.first_alternative();
        let first_span = self.current_span()?;

        let kind = match self.peek(0) {
            Keyword(Ref) => {
                self.next();
                FormalParamKind::Ref
            }
            _ => FormalParamKind::Value,
        };

        let name = self.name()?;
        self.commit();
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
            self.commit();
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

#[cfg(test)]
mod parser_spec;
