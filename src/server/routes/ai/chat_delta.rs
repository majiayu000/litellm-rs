use crate::core::types;

pub(super) fn convert_function_call_delta(
    function: types::responses::FunctionCallDelta,
) -> crate::core::streaming::types::FunctionCallDelta {
    crate::core::streaming::types::FunctionCallDelta {
        name: function.name,
        arguments: function.arguments,
    }
}

pub(super) fn convert_tool_call_delta(
    delta: types::responses::ToolCallDelta,
) -> crate::core::streaming::types::ToolCallDelta {
    crate::core::streaming::types::ToolCallDelta {
        index: delta.index,
        id: delta.id,
        tool_type: delta.tool_type,
        function: delta.function.map(
            |function| crate::core::streaming::types::FunctionCallDelta {
                name: function.name,
                arguments: function.arguments,
            },
        ),
    }
}
