use crate::messages::tool::tool_messages::line_tool::LineToolMessage;

// ...existing code...

fn handle_mouse_release(&mut self, input: &InputPreprocessor, responses: &mut VecDeque<Message>) {
    if self.current_tool == ToolType::Line {
        responses.add(LineToolMessage::DragStop.into());
    }

    // ...existing code...
}

// ...existing code...