use crate::ast::{BinaryOp, Binding, CallWhere, Case, Clause, Literal, Local, MatrixNode, Node, SortedCase, Syntax, UnaryOp, ValueNode};
use crate::constants::{Builtin, builtin_function, suffixed_function};
use crate::context::{FunctionProvider, NothingMap};
use crate::lexer::{names_matrix, offset_in};
use std::borrow::Cow;
use std::fmt;
use std::ops::Range;

/// A subexpression standing where its sort cannot, or a `where` clause with a cycle, a name defined twice, a call of the wrong
/// arity, or a definition of the sort its name's case forbids. Either fails the parse.
#[derive(Debug, Clone, PartialEq)]
pub struct SortError {
	message: Cow<'static, str>,
	/// The byte range of the source naming what the error blames, where it blames a name.
	span: Option<Range<usize>>,
}

impl SortError {
	const fn new(message: &'static str) -> Self {
		Self {
			message: Cow::Borrowed(message),
			span: None,
		}
	}

	pub fn span(&self) -> Option<Range<usize>> {
		self.span.clone()
	}

	/// Points the error at a name, which borrows directly from the source.
	fn blaming(self, source: &str, name: &str) -> Self {
		let span = offset_in(source, name).map(|start| start..start + name.len());
		Self { span, ..self }
	}
}

impl From<String> for SortError {
	fn from(message: String) -> Self {
		Self {
			message: Cow::Owned(message),
			span: None,
		}
	}
}

impl fmt::Display for SortError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(&self.message)
	}
}

const MATRIX_AS_VALUE: SortError = SortError::new("A matrix stands where a value is needed");
const VALUE_AS_MATRIX: SortError = SortError::new("A value stands where a matrix is needed");
const NO_MATRIX_OPERATOR: SortError = SortError::new("The operator has no meaning for a matrix");
const MIXED_CASES: SortError = SortError::new("A piecewise's cases must all be values or all be matrices");
const INVALID_ARGUMENTS: SortError = SortError::new("Invalid arguments for function call");

/// Reads the sort of every subexpression from its spelling, so each evaluator takes only trees of its own sort, and finds where
/// each name a `where` clause defines lives. A call of a function the host provides is a value, whatever builtin shares its name.
pub fn sorted<'src>(syntax: Syntax<'src>, source: &'src str, functions: &dyn FunctionProvider) -> Result<Node, SortError> {
	Sorter {
		functions,
		source,
		scopes: Vec::new(),
		uses: None,
		probe: None,
	}
	.sort(syntax)
}

/// How the sort pass read a name, for an editor to style it by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameUse {
	/// A called builtin or host function.
	Function,
	/// A function a `where` clause defines, called or being defined.
	LocalFunction,
	/// A value or matrix a `where` clause or a function's parameters define, read or being defined.
	Local,
	/// A name nothing in the expression defines, left to the host's bindings and the language's constants.
	Free,
}

/// How the sort pass reads each name of the source, by the byte offset where it begins. The pass reads on past an error, but a
/// name it can't place, like one in a clause whose definitions can't all be made, isn't recorded. The source is sorted with no host
/// functions, which is all the Math nodes supply.
pub(crate) fn name_uses<'src>(syntax: Syntax<'src>, source: &'src str) -> Vec<(usize, NameUse)> {
	let mut sorter = Sorter {
		functions: &NothingMap,
		source,
		scopes: Vec::new(),
		uses: Some(Vec::new()),
		probe: None,
	};
	let _ = sorter.sort(syntax);
	sorter.uses.unwrap_or_default()
}

/// A name defined around another, with its parameters where it's a function's.
pub(crate) type InScope<'src> = (&'src str, Option<Vec<&'src str>>);

/// What the sort pass finds around a name an editor asks about.
#[derive(Default)]
pub(crate) struct Probe<'src> {
	/// The byte offset where the name begins.
	offset: usize,
	/// The names the `where` clauses and parameters around it define, innermost first, once the name is read.
	pub names: Vec<InScope<'src>>,
	/// Whether the name is one a `where` clause or function's parameters are defining, rather than one being read.
	pub defining: bool,
}

/// The names the `where` clauses and parameters around a name define, innermost first, each function's with its parameters, for an
/// editor to offer while that name is typed, and whether the name is itself being defined. The name is found by the byte offset
/// where it begins, and nothing is found where the pass can't place it.
pub(crate) fn names_in_scope<'src>(syntax: Syntax<'src>, source: &'src str, offset: usize) -> Probe<'src> {
	let mut sorter = Sorter {
		functions: &NothingMap,
		source,
		scopes: Vec::new(),
		uses: Some(Vec::new()),
		probe: Some(Probe { offset, ..Probe::default() }),
	};
	let _ = sorter.sort(syntax);
	sorter.probe.unwrap_or_default()
}

/// Where a recorded name stands: read where it sees past the innermost `skip` scopes, or defined by a `where` clause or as a parameter.
#[derive(Clone, Copy)]
enum Site {
	Read { skip: usize },
	Definition,
}

/// One definition in a `where` clause, by its position among the clause's values or among its functions.
#[derive(Clone, Copy, PartialEq)]
enum Definition {
	Value(usize),
	Function(usize),
}

/// The names one scope defines: a `where` clause's values and functions, or a function's parameters, which are values.
#[derive(Default)]
struct Scope<'src> {
	values: Vec<&'src str>,
	functions: Vec<Function<'src>>,
	/// Whether the values are a function's parameters rather than a clause's definitions.
	holds_parameters: bool,
	/// The parameter read as the other sort than its case gives, while testing whether the function's body could take that sort.
	flipped: Option<usize>,
	/// The clause's definition being sorted, which depends on every other of the clause's definitions it reads.
	defining: Option<Definition>,
	/// Each definition paired with one it reads.
	dependencies: Vec<(Definition, Definition)>,
}

/// A function a `where` clause defines.
struct Function<'src> {
	name: &'src str,
	parameters: Vec<&'src str>,
	/// The unsorted body, sorted again only to test a call whose argument is the other sort than its parameter's case gives.
	body: Syntax<'src>,
}

impl<'src> Scope<'src> {
	fn parameters(parameters: Vec<&'src str>) -> Self {
		Self {
			values: parameters,
			holds_parameters: true,
			..Self::default()
		}
	}

	/// Adds a clause's definition, whose name may appear once among the clause's values and once among its functions, with no
	/// parameter named twice.
	fn define(&mut self, Binding { name, parameters, value }: &Binding<'src>, source: &str) -> Result<Definition, SortError> {
		let taken = if parameters.is_empty() {
			self.values.contains(name)
		} else {
			self.functions.iter().any(|function| function.name == *name)
		};
		if taken {
			return Err(SortError::from(format!("`{name}` is defined twice in one `where` clause")).blaming(source, name));
		}

		if let Some(parameter) = parameters
			.iter()
			.enumerate()
			.find_map(|(index, parameter)| parameters[..index].contains(parameter).then_some(parameter))
		{
			return Err(SortError::from(format!("`{name}` has two parameters named `{parameter}`")).blaming(source, parameter));
		}

		Ok(if parameters.is_empty() {
			self.values.push(name);
			Definition::Value(self.values.len() - 1)
		} else {
			self.functions.push(Function {
				name,
				parameters: parameters.clone(),
				body: value.clone(),
			});
			Definition::Function(self.functions.len() - 1)
		})
	}

	fn record_read(&mut self, definition: Definition) {
		if let Some(defining) = self.defining {
			self.dependencies.push((defining, definition));
		}
	}

	/// Rejects a definition that depends on itself, directly or through others, since its evaluation could never finish.
	/// The error points at the cycle's first definition.
	fn reject_cycles(&self, source: &str) -> Result<(), SortError> {
		// The values and then the functions, numbered as one list
		let position = |definition: Definition| match definition {
			Definition::Value(index) => index,
			Definition::Function(index) => self.values.len() + index,
		};
		let defined = self.values.iter().copied().chain(self.functions.iter().map(|function| function.name)).collect::<Vec<_>>();
		let names = defined.iter().map(|name| format!("`{name}`")).collect::<Vec<_>>();

		let mut reads = vec![Vec::new(); names.len()];
		for &(reader, read) in &self.dependencies {
			reads[position(reader)].push(position(read));
		}

		let Some(cycle) = find_cycle(&reads) else { return Ok(()) };
		let message = match cycle.as_slice() {
			[name] => format!("{} is defined in terms of itself", names[*name]),
			[first, second] => format!("{} and {} are defined in terms of each other", names[*first], names[*second]),
			[others @ .., last] => {
				let others = others.iter().map(|index| names[*index].as_str()).collect::<Vec<_>>().join(", ");
				format!("{others}, and {} are defined in terms of each other", names[*last])
			}
			[] => return Ok(()),
		};
		Err(SortError::from(message).blaming(source, defined[cycle[0]]))
	}
}

/// The first cycle found among nodes with the given successors, in the order its members lead to one another. O(nodes + edges).
fn find_cycle(successors: &[Vec<usize>]) -> Option<Vec<usize>> {
	#[derive(Clone, Copy, PartialEq)]
	enum Visit {
		Unvisited,
		OnPath,
		Finished,
	}

	fn visit(node: usize, successors: &[Vec<usize>], visits: &mut [Visit], path: &mut Vec<usize>) -> Option<Vec<usize>> {
		visits[node] = Visit::OnPath;
		path.push(node);

		for &next in &successors[node] {
			match visits[next] {
				Visit::OnPath => {
					let start = path.iter().position(|&member| member == next)?;
					return Some(path[start..].to_vec());
				}
				Visit::Unvisited => {
					if let Some(cycle) = visit(next, successors, visits, path) {
						return Some(cycle);
					}
				}
				Visit::Finished => {}
			}
		}

		path.pop();
		visits[node] = Visit::Finished;
		None
	}

	let mut visits = vec![Visit::Unvisited; successors.len()];
	let mut path = Vec::new();
	(0..successors.len()).find_map(|node| {
		if visits[node] == Visit::Unvisited {
			visit(node, successors, &mut visits, &mut path)
		} else {
			None
		}
	})
}

struct Sorter<'a, 'src> {
	functions: &'a dyn FunctionProvider,
	/// The source, which every name borrows from, so an error can point at the name it blames.
	source: &'src str,
	/// The scopes around the subexpression being sorted, innermost last.
	scopes: Vec<Scope<'src>>,
	/// How each name of the source was read, where an editor asked for them.
	uses: Option<Vec<(usize, NameUse)>>,
	/// The name whose surrounding definitions an editor asked for.
	probe: Option<Probe<'src>>,
}

impl<'src> Sorter<'_, 'src> {
	fn sort(&mut self, syntax: Syntax<'src>) -> Result<Node, SortError> {
		Ok(match syntax {
			Syntax::Lit(literal) => Node::Value(ValueNode::Lit(literal)),
			Syntax::Var(name) => self.name(name, 0),
			Syntax::FnCall { name, expr } => self.call(name, expr, 0)?,
			Syntax::BinOp { lhs, op, rhs } => {
				let lhs = self.sort(*lhs);
				let rhs = self.sort(*rhs);
				self.binary(lhs?, op, rhs?)?
			}
			Syntax::Product { first, rest } => {
				let mut rest = rest.into_iter();
				let product = self.sort(*first).and_then(|first| self.product(first, &mut rest));
				if product.is_err() {
					rest.for_each(|(_, factor)| {
						let _ = self.sort(factor);
					});
				}
				product?
			}
			Syntax::UnaryOp { expr, op } => match (self.sort(*expr)?, op) {
				(expr @ Node::Value(_), UnaryOp::Transpose) => return Err(self.misplaced(&expr, VALUE_AS_MATRIX)),
				(Node::Value(expr), op) => Node::Value(ValueNode::UnaryOp { expr: Box::new(expr), op }),
				(Node::Matrix(expr), UnaryOp::Pos | UnaryOp::Neg | UnaryOp::Transpose) => Node::Matrix(MatrixNode::UnaryOp { expr: Box::new(expr), op }),
				(expr @ Node::Matrix(_), _) => return Err(self.misplaced(&expr, NO_MATRIX_OPERATOR)),
			},
			Syntax::Comparison { first, rest } => {
				let first = self.sort(*first);
				let (operators, operands): (Vec<BinaryOp>, Vec<Syntax>) = rest.into_iter().unzip();
				let operands = self.sort_each(operands);
				let first = first?;
				let rest = operators
					.into_iter()
					.zip(operands)
					.map(|(op, operand)| Ok((op, operand?)))
					.collect::<Result<Vec<(BinaryOp, Node)>, SortError>>()?;
				match first {
					Node::Value(first) => {
						let rest = rest
							.into_iter()
							.map(|(op, operand)| Ok((op, self.value(operand, MATRIX_AS_VALUE)?)))
							.collect::<Result<Vec<_>, SortError>>()?;
						Node::Value(ValueNode::Comparison { first: Box::new(first), rest })
					}
					// Matrices have no order, so a chain over them is `==` or `!=` throughout
					Node::Matrix(first) => {
						let distinct = rest.iter().all(|(op, _)| *op == BinaryOp::Neq);
						if !distinct && !rest.iter().all(|(op, _)| *op == BinaryOp::Eq) {
							return Err(self.misplaced(&Node::Matrix(first), NO_MATRIX_OPERATOR));
						}
						let rest = rest.into_iter().map(|(_, operand)| self.matrix(operand, NO_MATRIX_OPERATOR));
						let matrices = std::iter::once(Ok(first)).chain(rest).collect::<Result<Vec<MatrixNode>, SortError>>()?;
						Node::Value(ValueNode::MatrixComparison { matrices, distinct })
					}
				}
			}
			Syntax::Piecewise { cases, otherwise } => self.piecewise(cases, otherwise)?,
			Syntax::Matrix { entries, by_rows } => Node::Matrix(MatrixNode::Literal {
				entries: self
					.sort_each(entries)
					.into_iter()
					.map(|entry| self.value(entry?, MATRIX_AS_VALUE))
					.collect::<Result<Vec<ValueNode>, SortError>>()?,
				by_rows,
			}),
			Syntax::Range { from, to } => {
				let from = self.sort(*from);
				let to = self.sort(*to);
				let (from, to) = (from?, to?);
				Node::Matrix(MatrixNode::Range {
					from: Box::new(self.value(from, MATRIX_AS_VALUE)?),
					to: Box::new(self.value(to, MATRIX_AS_VALUE)?),
				})
			}
			Syntax::Where { body, bindings } => self.clause(bindings, |sorter| sorter.sort(*body))?,
			// The function's name stands outside the parentheses holding the clause, so it looks past the clause's scope
			Syntax::CallWhere(call) => {
				let CallWhere { name, arguments, bindings } = *call;
				self.clause(bindings, |sorter| sorter.call(name, arguments, 1))?
			}
		})
	}

	/// Sorts every one of the subexpressions, even past one that fails, so an editor learns how they all read their names.
	fn sort_each(&mut self, syntaxes: Vec<Syntax<'src>>) -> Vec<Result<Node, SortError>> {
		syntaxes.into_iter().map(|syntax| self.sort(syntax)).collect()
	}

	/// A name read where the innermost scope defining it puts it, or else the host's binding or builtin of that spelling. The
	/// innermost `skip` scopes are passed over.
	fn name(&mut self, name: &'src str, skip: usize) -> Node {
		let local = self.local_value(name, skip);
		self.record(name, if local.is_some() { NameUse::Local } else { NameUse::Free }, Site::Read { skip });

		match local {
			Some(local) => local,
			None if names_matrix(name) => Node::Matrix(MatrixNode::Var(name.to_string())),
			None => Node::Value(ValueNode::Var(name.to_string())),
		}
	}

	/// Notes how a name of the source was read or defined, where an editor asked for it.
	fn record(&mut self, name: &str, name_use: NameUse, site: Site) {
		let Some(uses) = &mut self.uses else { return };
		let Some(offset) = offset_in(self.source, name) else { return };
		uses.push((offset, name_use));

		let Some(probe) = self.probe.as_mut().filter(|probe| probe.offset == offset) else { return };
		let skip = match site {
			Site::Read { skip } => skip,
			Site::Definition => {
				probe.defining = true;
				0
			}
		};

		// Every name defined around the probed one where it can read them, innermost first, once each, though a value and a function may share a name
		for scope in self.scopes.iter().rev().skip(skip) {
			// The definition being written can't read itself
			let unwritten = |definition: Definition| scope.defining != Some(definition);
			let values = scope
				.values
				.iter()
				.enumerate()
				.filter(|(index, _)| unwritten(Definition::Value(*index)))
				.map(|(_, value)| (*value, None));
			let functions = scope.functions.iter().enumerate().filter(|(index, _)| unwritten(Definition::Function(*index)));
			let functions = functions.map(|(_, function)| (function.name, Some(function.parameters.clone())));
			for defined in values.chain(functions) {
				if !probe.names.iter().any(|(name, parameters)| *name == defined.0 && parameters.is_some() == defined.1.is_some()) {
					probe.names.push(defined);
				}
			}
		}
	}

	/// A read of the value the innermost scope defining it puts, past the innermost `skip` scopes, which a `\` name never reaches.
	fn local_value(&mut self, name: &str, skip: usize) -> Option<Node> {
		if name.starts_with('\\') {
			return None;
		}

		self.scopes.iter_mut().rev().enumerate().skip(skip).find_map(|(depth, scope)| {
			let index = scope.values.iter().position(|value| *value == name)?;
			scope.record_read(Definition::Value(index));

			let local = Local { depth, index };
			Some(if names_matrix(name) != (scope.flipped == Some(index)) {
				Node::Matrix(MatrixNode::Local(local))
			} else {
				Node::Value(ValueNode::Local(local))
			})
		})
	}

	/// Where the innermost scope defining a function puts it, with its parameters, past the innermost `skip` scopes, which a `\`
	/// name never reaches.
	fn local_function(&mut self, name: &str, skip: usize) -> Option<(Local, Vec<&'src str>)> {
		if name.starts_with('\\') {
			return None;
		}

		self.scopes.iter_mut().rev().enumerate().skip(skip).find_map(|(depth, scope)| {
			let index = scope.functions.iter().position(|function| function.name == name)?;
			scope.record_read(Definition::Function(index));
			Some((Local { depth, index }, scope.functions[index].parameters.clone()))
		})
	}

	/// A call's sort follows the function's, where a `where` clause's function comes first and a matrix's name applies to its one
	/// argument. The name looks past the innermost `skip` scopes, which only its arguments see.
	fn call(&mut self, name: &'src str, arguments: Vec<Syntax<'src>>, skip: usize) -> Result<Node, SortError> {
		if let Some((function, parameters)) = self.local_function(name, skip) {
			self.record(name, NameUse::LocalFunction, Site::Read { skip });
			return self.local_call(name, function, &parameters, arguments);
		}

		let arguments = self.sort_each(arguments).into_iter().collect::<Result<Vec<Node>, SortError>>()?;
		let invalid_arguments = INVALID_ARGUMENTS.blaming(self.source, name);
		let (prefixed, bare_name) = match name.strip_prefix('\\') {
			Some(bare_name) => (true, bare_name),
			None => (false, name),
		};

		// A matrix applied to one argument is implicit multiplication, so `M(v)` matches `M v`
		if names_matrix(bare_name) {
			let matrix = self.name(name, skip);
			return self.binary(matrix, BinaryOp::Mul, one(arguments).ok_or(invalid_arguments)?);
		}

		// A host function shadows the builtin of its spelling unless the `\` prefix asks for the language's own
		let host_function = !prefixed && self.functions.provides(bare_name);
		let builtin = if host_function { None } else { builtin_function(bare_name) };

		// Where no function has the name, a value a `where` clause defines is implicit multiplication like any value, so `k(x + 1)` is `k (x + 1)`
		if !host_function
			&& builtin.is_none()
			&& suffixed_function(bare_name).is_none()
			&& let Some(local) = self.local_value(name, skip)
		{
			self.record(name, NameUse::Local, Site::Read { skip });
			return self.binary(local, BinaryOp::Mul, one(arguments).ok_or(invalid_arguments)?);
		}

		// Without a function of its name, the evaluator multiplies by a value the host binds to it
		let called = host_function || builtin.is_some() || suffixed_function(bare_name).is_some();
		self.record(name, if called { NameUse::Function } else { NameUse::Free }, Site::Read { skip });

		Ok(match builtin {
			Some(Builtin::OfMatrix(function)) => Node::Value(ValueNode::OfMatrix {
				function,
				matrix: Box::new(self.matrix(one(arguments).ok_or(invalid_arguments)?, VALUE_AS_MATRIX)?),
			}),
			Some(Builtin::MatrixOfMatrix(function)) => Node::Matrix(MatrixNode::OfMatrix {
				function,
				matrix: Box::new(self.matrix(one(arguments).ok_or(invalid_arguments)?, VALUE_AS_MATRIX)?),
			}),
			Some(Builtin::MatrixOfValues { function, arity }) => {
				if !arity.contains(&arguments.len()) {
					return Err(invalid_arguments);
				}
				Node::Matrix(MatrixNode::FromValues {
					function,
					arguments: self.values(arguments)?,
				})
			}
			Some(Builtin::OfValueAndRegions {
				function,
				regions: region_count,
				trailing_value,
			}) => {
				if !(region_count + 1..=region_count + 1 + usize::from(trailing_value)).contains(&arguments.len()) {
					return Err(invalid_arguments);
				}
				let mut arguments = arguments.into_iter();
				let value = Box::new(self.value(arguments.next().ok_or(invalid_arguments)?, MATRIX_AS_VALUE)?);
				let regions = arguments
					.by_ref()
					.take(region_count)
					.map(|region| self.matrix(region, VALUE_AS_MATRIX))
					.collect::<Result<Vec<MatrixNode>, SortError>>()?;
				let trailing = arguments.next().map(|trailing| self.value(trailing, MATRIX_AS_VALUE).map(Box::new)).transpose()?;
				Node::Value(ValueNode::OfValueAndRegions { function, value, regions, trailing })
			}
			_ => Node::Value(ValueNode::FnCall {
				name: name.to_string(),
				expr: self.values(arguments)?,
			}),
		})
	}

	/// A call of a function a `where` clause defines, taking one argument per parameter, each of the sort its parameter's spelling fixes.
	fn local_call(&mut self, name: &str, function: Local, parameters: &[&str], arguments: Vec<Syntax<'src>>) -> Result<Node, SortError> {
		if arguments.len() != parameters.len() {
			let count = parameters.len();
			let plural = if count == 1 { "" } else { "s" };
			return Err(SortError::from(format!("`{name}` takes {count} argument{plural}")).blaming(self.source, name));
		}

		let arguments = arguments
			.into_iter()
			.zip(parameters)
			.enumerate()
			.map(|(index, (argument, parameter))| {
				let argument = self.sort(argument)?;
				let passed_matrix = matches!(argument, Node::Matrix(_));
				if passed_matrix == names_matrix(parameter) {
					return Ok(argument);
				}

				// The parameter's case is the likelier mistake where the body could take what's passed, and the argument otherwise
				if self.takes_other_sort(function, index) {
					return Err(misnamed(parameter, &[], "passed", passed_matrix).blaming(self.source, parameter));
				}
				let (passed, needed) = if passed_matrix { ("matrix", "value") } else { ("value", "matrix") };
				let signature = format!("{name}({})", parameters.join(", "));
				let message = format!("`{signature}` is passed a {passed} for `{parameter}`, which its body uses as a {needed}");
				Err(SortError::from(message).blaming(self.source, name))
			})
			.collect::<Vec<_>>();
		let arguments = arguments.into_iter().collect::<Result<Vec<Node>, SortError>>()?;

		Ok(if names_matrix(name) {
			Node::Matrix(MatrixNode::Call { function, arguments })
		} else {
			Node::Value(ValueNode::Call { function, arguments })
		})
	}

	/// Whether a function's body sorts with one parameter read as the other sort than its case gives, as a call passing that sort needs.
	/// The body is sorted again where the function is defined, so this serves only a failing call.
	fn takes_other_sort(&mut self, function: Local, parameter: usize) -> bool {
		// One test at a time, since a tested body may make a failing call that would test again
		if self.scopes.iter().any(|scope| scope.flipped.is_some()) {
			return false;
		}

		let Some(definer) = self.scopes.len().checked_sub(function.depth + 1) else { return false };
		let Some(Function { parameters, body, .. }) = self.scopes[definer].functions.get(function.index) else {
			return false;
		};
		let (parameters, body) = (parameters.clone(), body.clone());

		// The test reads its names under a flipped parameter, so it records none of them
		let uses = self.uses.take();
		let inner_scopes = self.scopes.split_off(definer + 1);
		self.scopes.push(Scope {
			flipped: Some(parameter),
			..Scope::parameters(parameters)
		});
		let sorts = self.sort(body).is_ok();
		self.scopes.truncate(definer + 1);
		self.scopes.extend(inner_scopes);
		self.uses = uses;

		sorts
	}

	/// A product folds left, except that a matrix meeting a value applies to the whole rest of the product, so `M 2 v` is `M (2 v)`
	/// as in linear algebra rather than `(M 2) v`.
	fn product(&mut self, first: Node, rest: &mut std::vec::IntoIter<(BinaryOp, Syntax<'src>)>) -> Result<Node, SortError> {
		let mut accumulated = first;

		while let Some((op, factor)) = rest.next() {
			accumulated = match (accumulated, self.sort(factor)?) {
				(Node::Matrix(matrix), Node::Value(factor)) => {
					// Dividing by a value applies the matrix to its reciprocal times the rest, so `M / v w` is `M (v⁻¹ w)`
					let factor = if op == BinaryOp::Div { reciprocal(factor) } else { factor };

					let argument = self.product(Node::Value(factor), rest)?;
					return self.binary(Node::Matrix(matrix), BinaryOp::Mul, argument);
				}
				(accumulated, factor) => self.binary(accumulated, op, factor)?,
			};
		}

		Ok(accumulated)
	}

	/// A piecewise takes the sort of its cases, which must agree, under conditions that are values.
	fn piecewise(&mut self, cases: Vec<Case<'src>>, otherwise: Option<Box<Syntax<'src>>>) -> Result<Node, SortError> {
		let cases = cases
			.into_iter()
			.map(|Case { value: case, condition }| {
				let case = self.sort(case);
				let condition = self.sort(condition);
				Ok((case?, self.value(condition?, MATRIX_AS_VALUE)?))
			})
			.collect::<Vec<_>>();
		let otherwise = otherwise.map(|otherwise| self.sort(*otherwise));
		let cases = cases.into_iter().collect::<Result<Vec<(Node, ValueNode)>, SortError>>()?;
		let otherwise = otherwise.transpose()?;

		let first = cases.first().map(|(case, _)| case).or(otherwise.as_ref());
		if first.is_some_and(|first| matches!(first, Node::Matrix(_))) {
			let cases = cases
				.into_iter()
				.map(|(case, condition)| {
					Ok(SortedCase {
						value: self.matrix(case, MIXED_CASES)?,
						condition,
					})
				})
				.collect::<Result<Vec<_>, SortError>>()?;
			let otherwise = otherwise.map(|otherwise| self.matrix(otherwise, MIXED_CASES)).transpose()?.map(Box::new);
			return Ok(Node::Matrix(MatrixNode::Piecewise { cases, otherwise }));
		}

		let cases = cases
			.into_iter()
			.map(|(case, condition)| {
				Ok(SortedCase {
					value: self.value(case, MIXED_CASES)?,
					condition,
				})
			})
			.collect::<Result<Vec<_>, SortError>>()?;
		let otherwise = otherwise.map(|otherwise| self.value(otherwise, MIXED_CASES)).transpose()?.map(Box::new);
		Ok(Node::Value(ValueNode::Piecewise { cases, otherwise }))
	}

	/// A `where` clause and the body it ends, sorted within a scope holding every definition of the clause at once, so each may
	/// read any other whatever their order, as long as none depends on itself. The whole takes the sort of its body.
	fn clause(&mut self, bindings: Vec<Binding<'src>>, body: impl FnOnce(&mut Self) -> Result<Node, SortError>) -> Result<Node, SortError> {
		let mut scope = Scope::default();
		let definitions = bindings.iter().map(|binding| scope.define(binding, self.source)).collect::<Result<Vec<Definition>, SortError>>()?;
		self.scopes.push(scope);
		let depth = self.scopes.len();

		// Every definition is checked against its name's case before any read of it can fail, so a mismatch is blamed on the name,
		// not the read. A definition that fails to sort waits until the others are checked, since it may read a misnamed one.
		let mut clause = Clause {
			values: Vec::new(),
			functions: Vec::new(),
		};
		let (mut mismatch, mut failure) = (None, None);
		for (Binding { name, parameters, value }, definition) in bindings.into_iter().zip(definitions) {
			if let Some(scope) = self.scopes.last_mut() {
				scope.defining = Some(definition);
			}
			self.record(name, if parameters.is_empty() { NameUse::Local } else { NameUse::LocalFunction }, Site::Definition);
			parameters.iter().for_each(|parameter| self.record(parameter, NameUse::Local, Site::Definition));

			// A function's parameters are the innermost scope of its body, shadowing every other name
			let sorted = if parameters.is_empty() {
				self.sort(value).map(|sorted| (sorted, parameters))
			} else {
				self.scopes.push(Scope::parameters(parameters));
				self.sort(value).map(|sorted| (sorted, self.scopes.pop().unwrap_or_default().values))
			};

			match sorted.map(|(sorted, parameters)| defined_as(name, &parameters, sorted).map_err(|error| error.blaming(self.source, name))) {
				Ok(Ok(sorted)) => match definition {
					Definition::Value(_) => clause.values.push(sorted),
					Definition::Function(_) => clause.functions.push(sorted),
				},
				Ok(Err(error)) => {
					mismatch.get_or_insert(error);
				}
				Err(error) => {
					self.scopes.truncate(depth);
					failure.get_or_insert(error);
				}
			}
		}

		// The body reads the definitions without being one of them, and is sorted even after a definition fails so an editor learns
		// how it reads its names
		if let Some(scope) = self.scopes.last_mut() {
			scope.defining = None;
		}
		let cycles = match (&mismatch, &failure, self.scopes.last()) {
			(None, None, Some(scope)) => scope.reject_cycles(self.source),
			_ => Ok(()),
		};
		let body = body(self);
		self.scopes.truncate(depth - 1);

		if let Some(error) = mismatch.or(failure) {
			return Err(error);
		}
		cycles?;
		let body = body?;

		let clause = Box::new(clause);
		Ok(match body {
			Node::Value(body) => Node::Value(ValueNode::Where { clause, body: Box::new(body) }),
			Node::Matrix(body) => Node::Matrix(MatrixNode::Where { clause, body: Box::new(body) }),
		})
	}

	/// The error for a node of the wrong sort, which names the parameter the node reads, if it's one, instead of `rejected`.
	/// A `where` definition's name is left to its clause, which blames it only where its definition disagrees with it.
	fn misplaced(&self, node: &Node, rejected: SortError) -> SortError {
		let (Node::Value(ValueNode::Local(local)) | Node::Matrix(MatrixNode::Local(local))) = node else {
			return rejected;
		};
		let parameter = self
			.scopes
			.iter()
			.rev()
			.nth(local.depth)
			.filter(|scope| scope.holds_parameters)
			.and_then(|scope| scope.values.get(local.index));

		match parameter {
			Some(parameter) => misnamed(parameter, &[], "used as", matches!(node, Node::Value(_))).blaming(self.source, parameter),
			None => rejected,
		}
	}

	fn value(&self, node: Node, rejected: SortError) -> Result<ValueNode, SortError> {
		match node {
			Node::Value(value) => Ok(value),
			node => Err(self.misplaced(&node, rejected)),
		}
	}

	fn matrix(&self, node: Node, rejected: SortError) -> Result<MatrixNode, SortError> {
		match node {
			Node::Matrix(matrix) => Ok(matrix),
			node => Err(self.misplaced(&node, rejected)),
		}
	}

	fn values(&self, nodes: Vec<Node>) -> Result<Vec<ValueNode>, SortError> {
		nodes.into_iter().map(|node| self.value(node, MATRIX_AS_VALUE)).collect()
	}

	/// The sort of an operation: value·value and matrix·value are values, matrix·matrix and value·matrix are matrices, and
	/// `+` joins like sorts or attaches a value to a matrix as translation.
	fn binary(&self, lhs: Node, op: BinaryOp, rhs: Node) -> Result<Node, SortError> {
		use BinaryOp as Op;
		Ok(match (lhs, op, rhs) {
			(Node::Value(lhs), op, Node::Value(rhs)) => Node::Value(ValueNode::BinOp {
				lhs: Box::new(lhs),
				op,
				rhs: Box::new(rhs),
			}),
			(Node::Matrix(matrix), Op::Mul, Node::Value(value)) => Node::Value(ValueNode::Apply {
				matrix: Box::new(matrix),
				value: Box::new(value),
			}),
			// Division is times-inverse, so a matrix over a value applies to the value's reciprocal
			(Node::Matrix(matrix), Op::Div, Node::Value(value)) => Node::Value(ValueNode::Apply {
				matrix: Box::new(matrix),
				value: Box::new(reciprocal(value)),
			}),
			(Node::Matrix(lhs), Op::Eq | Op::Neq, Node::Matrix(rhs)) => Node::Value(ValueNode::MatrixComparison {
				matrices: vec![lhs, rhs],
				distinct: op == Op::Neq,
			}),
			(lhs @ Node::Value(_), Op::Mul | Op::Div | Op::Add | Op::Sub, rhs @ Node::Matrix(_))
			| (lhs @ Node::Matrix(_), Op::Mul | Op::Div | Op::Add | Op::Sub, rhs @ Node::Matrix(_))
			| (lhs @ Node::Matrix(_), Op::Add | Op::Sub | Op::Pow, rhs @ Node::Value(_)) => Node::Matrix(MatrixNode::BinOp {
				lhs: Box::new(lhs),
				op,
				rhs: Box::new(rhs),
			}),
			// Where only one operand is a matrix, it's the one the operator can't take
			(lhs, _, rhs) => {
				let matrix = match (lhs, rhs) {
					(matrix @ Node::Matrix(_), Node::Value(_)) | (Node::Value(_), matrix @ Node::Matrix(_)) => matrix,
					_ => return Err(NO_MATRIX_OPERATOR),
				};
				return Err(self.misplaced(&matrix, NO_MATRIX_OPERATOR));
			}
		})
	}
}

/// A definition of the sort its name's spelling fixes, a matrix for a name beginning with a capital letter and a value otherwise.
fn defined_as(name: &str, parameters: &[&str], definition: Node) -> Result<Node, SortError> {
	let is_matrix = matches!(definition, Node::Matrix(_));
	if names_matrix(name) == is_matrix {
		return Ok(definition);
	}
	Err(misnamed(name, parameters, "defined as", is_matrix))
}

/// Advice to recase a `where` name or parameter defined, used, or passed as the other sort, since its case is the likelier
/// mistake, with a function quoted with its parameters. The recased name is offered where recasing gives that sort.
fn misnamed(name: &str, parameters: &[&str], treated: &str, as_matrix: bool) -> SortError {
	let (sort, letter) = if as_matrix { ("matrix", "capital") } else { ("value", "lowercase") };
	let mut message = if parameters.is_empty() {
		format!("`{name}` is {treated} a {sort}, so rename it to begin with a {letter} letter")
	} else {
		format!("`{name}({})` is {treated} a {sort}, so rename `{name}` to begin with a {letter} letter", parameters.join(", "))
	};

	let mut characters = name.chars();
	let recased: String = match characters.next() {
		Some(first) if as_matrix => first.to_uppercase().chain(characters).collect(),
		Some(first) => first.to_lowercase().chain(characters).collect(),
		None => String::new(),
	};
	if names_matrix(&recased) == as_matrix {
		message.push_str(&format!(", like `{recased}`"));
	}

	SortError::from(message)
}

fn one(arguments: Vec<Node>) -> Option<Node> {
	<[Node; 1]>::try_from(arguments).ok().map(|[argument]| argument)
}

fn reciprocal(value: ValueNode) -> ValueNode {
	ValueNode::BinOp {
		lhs: Box::new(ValueNode::Lit(Literal::Integer(1))),
		op: BinaryOp::Div,
		rhs: Box::new(value),
	}
}
