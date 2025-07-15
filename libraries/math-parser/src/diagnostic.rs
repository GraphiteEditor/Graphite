use std::io::{self, Write};

use codespan_reporting::{
	diagnostic::{Diagnostic, Label, LabelStyle, Severity},
	files::{self, Files, SimpleFile, SimpleFiles},
	term::{
		self, Config, Renderer, RichDiagnostic, emit,
		termcolor::{ColorChoice, StandardStream},
	},
};

use crate::lexer::Span;

pub struct CompileError {
	pub file: SimpleFile<String, String>,
	pub diagnostics: Vec<Diagnostic<()>>,
}

impl CompileError {
	pub fn print(&self) {
		let mut writer = StandardStream::stderr(ColorChoice::Auto);
		let config = term::Config::default();
		for diag in &self.diagnostics {
			term::emit(&mut writer.lock(), &config, &self.file, diag).unwrap();
		}
		writer.flush();
	}

	pub fn render_html(&self, config: &Config) -> Result<Vec<u8>, files::Error> {
		let mut buf = Vec::new();
		{
			let mut html_writer = HtmlWriter::new(&mut buf);

			let mut renderer = Renderer::new(&mut html_writer, config);
			for diag in &self.diagnostics {
				RichDiagnostic::new(diag, config).render(&self.file, &mut renderer)?;
			}
			html_writer.close_span().expect("buffer writer cant fail");
		}

		Ok(buf)
	}
}

pub(crate) fn make_compile_error(filename: impl Into<String>, src: &str, errs: impl IntoIterator<Item = (String, Span, Vec<(String, Span)>)>) -> CompileError {
	let file = SimpleFile::new(filename.into(), src.to_string());

	let diagnostics = errs.into_iter().map(|(msg, primary, secondaries)| make_diagnostic(msg, primary, &secondaries)).collect();

	CompileError { file, diagnostics }
}

fn make_diagnostic(msg: impl Into<String>, primary: Span, secondaries: &[(String, Span)]) -> Diagnostic<()> {
	let msg_str = msg.into();
	let mut labels = vec![Label::primary((), primary).with_message(msg_str.clone())];
	for (smsg, span) in secondaries {
		labels.push(Label::secondary((), *span).with_message(smsg.clone()));
	}
	Diagnostic::error().with_message(msg_str).with_labels(labels)
}

struct HtmlWriter<W> {
	upstream: W,
	span_open: bool,
}

impl<W: Write> HtmlWriter<W> {
	pub fn new(upstream: W) -> Self {
		HtmlWriter { upstream, span_open: false }
	}

	/// Close any open span
	fn close_span(&mut self) -> io::Result<()> {
		if self.span_open {
			write!(self.upstream, "</span>")?;
			self.span_open = false;
		}
		Ok(())
	}

	/// Open a new span with the given CSS class
	fn open_span(&mut self, class: &str) -> io::Result<()> {
		// close existing first
		self.close_span()?;
		write!(self.upstream, "<span class=\"{}\">", class)?;
		self.span_open = true;
		Ok(())
	}
}

impl<W: Write> Write for HtmlWriter<W> {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		let mut last = 0;
		for (i, &b) in buf.iter().enumerate() {
			let escape = match b {
				b'<' => b"&lt;"[..].as_ref(),
				b'>' => b"&gt;"[..].as_ref(),
				b'&' => b"&amp;"[..].as_ref(),
				_ => continue,
			};
			self.upstream.write_all(&buf[last..i])?;
			self.upstream.write_all(escape)?;
			last = i + 1;
		}
		self.upstream.write_all(&buf[last..])?;
		Ok(buf.len())
	}
	fn flush(&mut self) -> io::Result<()> {
		self.upstream.flush()
	}
}

impl<W: Write> codespan_reporting::term::WriteStyle for HtmlWriter<W> {
	fn set_header(&mut self, severity: Severity) -> io::Result<()> {
		let class = match severity {
			Severity::Bug => "header-bug",
			Severity::Error => "header-error",
			Severity::Warning => "header-warning",
			Severity::Note => "header-note",
			Severity::Help => "header-help",
		};
		self.open_span(class)
	}

	fn set_header_message(&mut self) -> io::Result<()> {
		self.open_span("header-message")
	}

	fn set_line_number(&mut self) -> io::Result<()> {
		self.open_span("line-number")
	}

	fn set_note_bullet(&mut self) -> io::Result<()> {
		self.open_span("note-bullet")
	}

	fn set_source_border(&mut self) -> io::Result<()> {
		self.open_span("source-border")
	}

	fn set_label(&mut self, severity: Severity, label_style: LabelStyle) -> io::Result<()> {
		let sev = match severity {
			Severity::Bug => "bug",
			Severity::Error => "error",
			Severity::Warning => "warning",
			Severity::Note => "note",
			Severity::Help => "help",
		};
		let typ = match label_style {
			LabelStyle::Primary => "primary",
			LabelStyle::Secondary => "secondary",
		};
		self.open_span(&format!("label-{}-{}", typ, sev))
	}

	fn reset(&mut self) -> io::Result<()> {
		self.close_span()
	}
}
