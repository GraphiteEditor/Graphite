use editor::messages::message::Message;
use editor::utility_types::DebugMessageTree;
use std::io::Write;
use std::path::PathBuf;

const FRONTEND_MESSAGE_STR: &str = "FrontendMessage";

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let output_dir = std::env::args_os().nth(1).map(PathBuf::from).ok_or("Usage: editor-message-tree <output-directory>")?;
	std::fs::create_dir_all(&output_dir)?;

	let tree = Message::message_tree();

	// Write the .txt file (plain text tree outline, served as a static download)
	let static_dir = output_dir.join("../static/volunteer/guide/codebase-overview");
	std::fs::create_dir_all(&static_dir)?;
	let mut txt_file = std::fs::File::create(static_dir.join("hierarchical-message-system-tree.txt"))?;
	write_tree_txt(&tree, &mut txt_file)?;

	// Write the .html file (structured HTML embedded in the website page)
	let mut html = String::new();
	write_tree_html(&tree, &mut html);
	std::fs::write(output_dir.join("hierarchical-message-system-tree.html"), &html)?;

	Ok(())
}

// =================
// PLAIN TEXT OUTPUT
// =================

fn write_tree_txt(tree: &DebugMessageTree, file: &mut std::fs::File) -> std::io::Result<()> {
	if tree.path().is_empty() {
		file.write_all(format!("{}\n", tree.name()).as_bytes())?;
	} else {
		file.write_all(format!("{} `{}#L{}`\n", tree.name(), tree.path(), tree.line_number()).as_bytes())?;
	}

	if let Some(variants) = tree.variants() {
		for (i, variant) in variants.iter().enumerate() {
			let is_last = i == variants.len() - 1;
			write_tree_txt_node(variant, "", is_last, file)?;
		}
	}

	Ok(())
}

fn write_tree_txt_node(tree: &DebugMessageTree, prefix: &str, is_last: bool, file: &mut std::fs::File) -> std::io::Result<()> {
	let (branch, child_prefix) = if tree.message_handler_data_fields().is_some() || tree.message_handler_fields().is_some() {
		("├── ", format!("{prefix}│   "))
	} else if is_last {
		("└── ", format!("{prefix}    "))
	} else {
		("├── ", format!("{prefix}│   "))
	};

	if tree.path().is_empty() {
		file.write_all(format!("{}{}{}\n", prefix, branch, tree.name()).as_bytes())?;
	} else {
		file.write_all(format!("{}{}{} `{}#L{}`\n", prefix, branch, tree.name(), tree.path(), tree.line_number()).as_bytes())?;
	}

	if let Some(variants) = tree.variants() {
		let len = variants.len();
		for (i, variant) in variants.iter().enumerate() {
			let is_last_child = i == len - 1;
			write_tree_txt_node(variant, &child_prefix, is_last_child, file)?;
		}
	}

	if let Some(fields) = tree.fields() {
		let len = fields.len();
		for (i, field) in fields.iter().enumerate() {
			let is_last_field = i == len - 1;
			let branch = if is_last_field { "└── " } else { "├── " };
			file.write_all(format!("{child_prefix}{branch}{field}\n").as_bytes())?;
		}
	}

	if let Some(data) = tree.message_handler_fields() {
		let len = data.fields().len();
		let (branch, child_prefix) = if tree.message_handler_data_fields().is_some() {
			("├── ", format!("{prefix}│   "))
		} else {
			("└── ", format!("{prefix}    "))
		};

		if data.name().is_empty() && tree.name() != FRONTEND_MESSAGE_STR {
			panic!("{}'s MessageHandler is missing #[message_handler_data]", tree.name());
		} else if tree.name() != FRONTEND_MESSAGE_STR {
			file.write_all(format!("{}{}{} `{}#L{}`\n", prefix, branch, data.name(), data.path(), data.line_number()).as_bytes())?;

			for (i, field) in data.fields().iter().enumerate() {
				let is_last_field = i == len - 1;
				let branch = if is_last_field { "└── " } else { "├── " };
				file.write_all(format!("{}{}{}\n", child_prefix, branch, field.0).as_bytes())?;
			}
		}
	}

	if let Some(data) = tree.message_handler_data_fields() {
		let len = data.fields().len();
		if data.path().is_empty() {
			file.write_all(format!("{}{}{}\n", prefix, "└── ", data.name()).as_bytes())?;
		} else {
			file.write_all(format!("{}{}{} `{}#L{}`\n", prefix, "└── ", data.name(), data.path(), data.line_number()).as_bytes())?;
		}
		for (i, field) in data.fields().iter().enumerate() {
			let is_last_field = i == len - 1;
			let branch = if is_last_field { "└── " } else { "├── " };
			let field = &field.0;
			file.write_all(format!("{prefix}    {branch}{field}\n").as_bytes())?;
		}
	}

	Ok(())
}

// ===========
// HTML OUTPUT
// ===========

const GITHUB_BASE: &str = "https://github.com/GraphiteEditor/Graphite/blob/master/";
const NAMING_SUFFIXES: &[&str] = &["Message", "MessageHandler", "MessageContext"];

fn escape_html(s: &str) -> String {
	s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn github_link(path: &str, line: usize) -> String {
	let path = path.replace('\\', "/");
	let filename = path.rsplit('/').next().unwrap_or(&path);
	format!(r#"<a href="{GITHUB_BASE}{path}#L{line}" target="_blank">{filename}:{line}</a>"#)
}

fn naming_convention_warning(name: &str) -> &'static str {
	// Strip generic parameters for the check (e.g. `Foo<Bar>` -> `Foo`)
	let base_name = name.split('<').next().unwrap_or(name);
	if NAMING_SUFFIXES.iter().any(|suffix| base_name.ends_with(suffix)) {
		""
	} else {
		r#"<span class="warn">(violates naming convention — should end with 'Message', 'MessageHandler', or 'MessageContext')</span>"#
	}
}

fn write_tree_html(tree: &DebugMessageTree, out: &mut String) {
	// Root node
	let link = if !tree.path().is_empty() { github_link(tree.path(), tree.line_number()) } else { String::new() };
	let escaped_name = escape_html(tree.name());

	out.push_str("<ul>\n");
	out.push_str(&format!(r#"<li><span class="tree-node"><span class="subsystem">{escaped_name}</span>{link}</span>"#));

	if let Some(variants) = tree.variants() {
		out.push_str(r#"<div class="nested">"#);
		write_tree_html_children(variants, out);
		out.push_str("</div>");
	}

	out.push_str("</li>\n</ul>\n");
}

fn write_tree_html_children(variants: &[DebugMessageTree], out: &mut String) {
	out.push_str("<ul>\n");
	for variant in variants {
		write_tree_html_node(variant, out);
	}
	out.push_str("</ul>\n");
}

fn write_tree_html_node(tree: &DebugMessageTree, out: &mut String) {
	let has_link = !tree.path().is_empty();
	let link = if has_link { github_link(tree.path(), tree.line_number()) } else { String::new() };
	let escaped_name = escape_html(tree.name());

	enum HtmlChild<'a> {
		Subtree(&'a DebugMessageTree),
		Field(String),
		HandlerFields(String, String, usize, Vec<String>),
		DataFields(String, String, usize, Vec<String>),
	}

	// Collect all child entries for this node
	let mut children: Vec<HtmlChild> = Vec::new();

	if let Some(variants) = tree.variants() {
		for variant in variants {
			children.push(HtmlChild::Subtree(variant));
		}
	}

	if let Some(fields) = tree.fields() {
		for field in fields {
			children.push(HtmlChild::Field(field.to_string()));
		}
	}

	if let Some(data) = tree.message_handler_fields()
		&& (!data.name().is_empty() || tree.name() == FRONTEND_MESSAGE_STR)
		&& tree.name() != FRONTEND_MESSAGE_STR
	{
		children.push(HtmlChild::HandlerFields(
			data.name().to_string(),
			data.path().to_string(),
			data.line_number(),
			data.fields().iter().map(|f| f.0.clone()).collect(),
		));
	}

	if let Some(data) = tree.message_handler_data_fields() {
		children.push(HtmlChild::DataFields(
			data.name().to_string(),
			data.path().to_string(),
			data.line_number(),
			data.fields().iter().map(|f| f.0.clone()).collect(),
		));
	}

	let has_children = !children.is_empty();
	let has_deeper_children = children.iter().any(|child| matches!(child, HtmlChild::Subtree(t) if t.variants().is_some() || t.fields().is_some()));

	// Determine role
	let role = if has_link {
		"subsystem"
	} else if has_deeper_children {
		"submessage"
	} else {
		"message"
	};

	// Naming convention warning (only for linked/subsystem nodes)
	let warning = if has_link { naming_convention_warning(tree.name()) } else { "" };

	if has_children {
		out.push_str(&format!(r#"<li><span class="tree-node"><span class="{role}">{escaped_name}</span>{link}{warning}</span>"#));
		out.push_str(r#"<div class="nested"><ul>"#);
		out.push('\n');

		for child in &children {
			match child {
				HtmlChild::Subtree(subtree) => write_tree_html_node(subtree, out),
				HtmlChild::Field(field) => write_field_html(field, out),
				HtmlChild::HandlerFields(name, path, line, fields) => write_handler_or_data_html(name, path, *line, fields, out),
				HtmlChild::DataFields(name, path, line, fields) => write_handler_or_data_html(name, path, *line, fields, out),
			}
		}

		out.push_str("</ul>\n</div></li>\n");
	} else {
		out.push_str(&format!(r#"<li><span class="tree-leaf {role}">{escaped_name}</span>{link}{warning}</li>"#));
		out.push('\n');
	}
}

fn write_field_html(field: &str, out: &mut String) {
	if let Some((name, ty)) = field.split_once(':') {
		let name = escape_html(name.trim());
		let ty = escape_html(ty.trim());
		out.push_str(&format!(r#"<li><span class="tree-leaf field">{name}</span>: <span>{ty}</span></li>"#));
	} else {
		let escaped = escape_html(field);
		out.push_str(&format!(r#"<li><span class="tree-leaf message">{escaped}</span></li>"#));
	}
	out.push('\n');
}

fn write_handler_or_data_html(name: &str, path: &str, line: usize, fields: &[String], out: &mut String) {
	let escaped_name = escape_html(name);
	let link = if !path.is_empty() { github_link(path, line) } else { String::new() };
	let warning = if !path.is_empty() { naming_convention_warning(name) } else { "" };

	if fields.is_empty() {
		out.push_str(&format!(r#"<li><span class="tree-leaf subsystem">{escaped_name}</span>{link}{warning}</li>"#));
	} else {
		out.push_str(&format!(r#"<li><span class="tree-node"><span class="subsystem">{escaped_name}</span>{link}{warning}</span>"#));
		out.push_str(r#"<div class="nested"><ul>"#);
		out.push('\n');
		for field in fields {
			write_field_html(field, out);
		}
		out.push_str("</ul>\n</div></li>\n");
	}
}
