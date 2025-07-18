use oxc_ast::{
    AstKind,
    ast::{Argument, Expression, FormalParameter},
};
use oxc_span::Atom;

/// Check if the given node is registering an endpoint handler or middleware to
/// a route or Express application object. If it is, it
/// returns:
/// - the endpoint path being handled, if found and statically analyzable
/// - the arguments to the handler function, excluding the path (if found)
///
/// ## Example
/// ```js
///
/// app.get('/path', (req, res) => { }); // -> Some(( Some("/path"), [Argument::Expression(Expression::Function(...))] ))
/// app.use(someMiddleware);             // -> Some(( None, [Argument::Expression(Expression::IdentifierReference)] ))
///
/// ```
pub fn as_endpoint_registration<'a, 'n>(
    node: &'n AstKind<'a>,
) -> Option<(Option<Atom<'a>>, &'n [Argument<'a>])> {
    let call = node.as_call_expression()?;
    let callee = call.callee.as_member_expression()?;
    let method_name = callee.static_property_name()?;
    if ROUTER_HANDLER_METHOD_NAMES.binary_search(&method_name).is_err() {
        return None;
    }
    if call.arguments.is_empty() {
        return None;
    }
    let first = call.arguments[0].as_expression()?;
    match first {
        Expression::StringLiteral(path) => {
            Some((Some(path.value), &call.arguments.as_slice()[1..]))
        }
        Expression::TemplateLiteral(template) => {
            template.single_quasi().map(|quasi| (Some(quasi), &call.arguments.as_slice()[1..]))
        }
        _ => Some((None, call.arguments.as_slice())),
    }
}

/// Check if the given expression is an endpoint handler function.
///
/// This will yield a lot of false positives if not called on the results of
/// [`as_endpoint_registration`].
pub fn is_endpoint_handler(maybe_handler: &Expression<'_>) -> bool {
    let params = match maybe_handler {
        Expression::FunctionExpression(f) => &f.params,
        Expression::ArrowFunctionExpression(arrow) => &arrow.params,
        _ => return false,
    };

    // NOTE(@DonIsaac): should we check for destructuring patterns? I don't
    // really ever see them used in handlers, and their existence could indicate
    // this function is not a handler.
    if params.rest.is_some() {
        return false;
    }
    match params.items.as_slice() {
        [req] => is_req_param(req),
        [req, res] => is_req_param(req) && is_res_param(res),
        [req, res, next] => {
            is_req_param(req) && is_res_param(res) && is_next_param(next) ||
                // (err, req, res)
                is_error_param(req) && is_req_param(res) && is_res_param(next)
        }
        [err, req, res, next] => {
            is_error_param(err) && is_req_param(req) && is_res_param(res) && is_next_param(next)
        }
        _ => false,
    }
}

const ROUTER_HANDLER_METHOD_NAMES: [&str; 9] =
    ["all", "delete", "get", "head", "options", "patch", "post", "put", "use"];

const COMMON_REQUEST_NAMES: [&str; 3] = ["r", "req", "request"];
fn is_req_param(param: &FormalParameter) -> bool {
    param
        .pattern
        .get_identifier_name()
        .is_some_and(|id| COMMON_REQUEST_NAMES.contains(&id.as_str()))
}

const COMMON_RESPONSE_NAMES: [&str; 3] = ["s", "res", "response"];
fn is_res_param(param: &FormalParameter) -> bool {
    param
        .pattern
        .get_identifier_name()
        .is_some_and(|id| COMMON_RESPONSE_NAMES.contains(&id.as_str()))
}

const COMMON_NEXT_NAMES: [&str; 2] = ["n", "next"];
fn is_next_param(param: &FormalParameter) -> bool {
    param.pattern.get_identifier_name().is_some_and(|id| COMMON_NEXT_NAMES.contains(&id.as_str()))
}

const COMMON_ERROR_NAMES: [&str; 4] = ["e", "err", "error", "exception"];
fn is_error_param(param: &FormalParameter) -> bool {
    param.pattern.get_identifier_name().is_some_and(|id| COMMON_ERROR_NAMES.contains(&id.as_str()))
}
