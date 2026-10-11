use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{
    AnonStructType, AnonStructValue, Format, IntegerValue, PrimitiveIntegerValue, StructValue,
    Type, Value,
};
use fpp_ast::{EventThrottle, Expr, FormalParam, IntegerKind, SpecEvent};
use fpp_core::{Span, Spanned};
use std::sync::Arc;

/// An FPP event
#[derive(Debug, Clone)]
pub struct Event {
    pub node: Arc<SpecEvent>,
    pub format: Format,
    pub throttle: Option<Throttle>,
}

impl Event {
    /// Gets the name of the event
    pub fn get_name(&self) -> &str {
        &self.node.name.data
    }

    /// Gets the location of the event
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// Creates an event from an event specifier
    pub fn from_spec_event(a: &Analysis, node: &SpecEvent) -> SemanticResult<Event> {
        let loc = node.span();
        let check_ref_params = |params: &[FormalParam]| {
            if Analysis::get_num_ref_params(params) != 0 {
                Err(SemanticError::InvalidEvent {
                    loc,
                    msg: "event may not have ref parameters".to_string(),
                })
            } else {
                Ok(())
            }
        };
        check_ref_params(&node.params)?;
        a.check_displayable_params(&node.params, "type of event is not displayable")?;
        let types: Vec<(Arc<Type>, Span)> = node
            .params
            .iter()
            .filter_map(|p| {
                a.type_map
                    .get(&p.type_name.node_id)
                    .map(|t| (t.clone(), p.type_name.span()))
            })
            .collect();
        let format = Format::new(&node.format, types);
        let throttle = match &node.throttle {
            Some(throttle) => Some(check_event_throttle(a, throttle, loc)?),
            None => None,
        };
        Ok(Event {
            node: Arc::new(node.clone()),
            format,
            throttle,
        })
    }
}

/// A time interval, as used in an event throttle
#[derive(Debug, Clone)]
pub struct TimeInterval {
    pub seconds: i64,
    pub useconds: i32,
}

/// An event throttle
#[derive(Debug, Clone)]
pub struct Throttle {
    pub count: i32,
    pub every: Option<TimeInterval>,
}

/// Gets the time interval of an event throttle.
///
/// Returns `None` when the interval expression has no value of the shape
/// `{ seconds: U32, useconds: U32 }`. That can only happen on an expression
/// whose error CheckExprTypes already reported, so there is nothing to add here;
/// the only diagnostics from here are the range checks.
fn get_every_interval_value(a: &Analysis, every: &Expr) -> SemanticResult<Option<TimeInterval>> {
    let loc = every.span();
    let u32_type = Arc::new(Type::PrimitiveInt(IntegerKind::U32));
    let interval_type = Arc::new(Type::AnonStruct(AnonStructType {
        members: vec![
            ("seconds".to_string(), u32_type.clone()),
            ("useconds".to_string(), u32_type.clone()),
        ],
    }));
    let Some(value) = a.value_map.get(&every.node_id) else {
        return Ok(None);
    };
    let interval_value = match value.convert(&interval_type) {
        Some(Value::Struct(StructValue { anon_struct, .. }))
        | Some(Value::AnonStruct(anon_struct)) => anon_struct,
        _ => return Ok(None),
    };
    let Some(seconds) = get_member(&interval_value, "seconds", u32::MAX as i128, loc)? else {
        return Ok(None);
    };
    let Some(useconds) = get_member(&interval_value, "useconds", 999_999, loc)? else {
        return Ok(None);
    };
    Ok(Some(TimeInterval {
        seconds: seconds as i64,
        useconds: useconds as i32,
    }))
}

/// Gets one member of a throttle interval, checking it against its range.
/// Returns `None` if the member is absent or not an integer; see
/// [`get_every_interval_value`].
fn get_member(
    interval_value: &AnonStructValue,
    member: &str,
    max_value: i128,
    loc: Span,
) -> SemanticResult<Option<i128>> {
    let u32_type = Arc::new(Type::PrimitiveInt(IntegerKind::U32));
    let v = match interval_value
        .members
        .get(member)
        .and_then(|v| v.convert(&u32_type))
    {
        Some(Value::PrimitiveInteger(PrimitiveIntegerValue { value, .. }))
        | Some(Value::Integer(IntegerValue(value))) => value,
        _ => return Ok(None),
    };
    if v < 0 || v > max_value {
        return Err(SemanticError::InvalidIntValue {
            loc,
            v: Some(v),
            msg: format!("{} must be in the range [0, {}]", member, max_value),
        });
    }
    Ok(Some(v))
}

/// Checks an event throttle
fn check_event_throttle(
    a: &Analysis,
    throttle: &EventThrottle,
    loc: Span,
) -> SemanticResult<Throttle> {
    let count = a.get_nonnegative_int_value(throttle.count.node_id, throttle.count.span())?;
    if count == 0 {
        return Err(SemanticError::InvalidEvent {
            loc,
            msg: "event throttle count must be greater than zero".to_string(),
        });
    }
    let every = match &throttle.every {
        Some(every) => get_every_interval_value(a, every)?,
        None => None,
    };
    Ok(Throttle {
        count: count as i32,
        every,
    })
}
