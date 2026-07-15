import llm_pb2 as _llm_pb2
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class ToolCall(_message.Message):
    __slots__ = ("tool", "args_json", "scope")
    TOOL_FIELD_NUMBER: _ClassVar[int]
    ARGS_JSON_FIELD_NUMBER: _ClassVar[int]
    SCOPE_FIELD_NUMBER: _ClassVar[int]
    tool: str
    args_json: str
    scope: str
    def __init__(self, tool: _Optional[str] = ..., args_json: _Optional[str] = ..., scope: _Optional[str] = ...) -> None: ...

class ToolResult(_message.Message):
    __slots__ = ("content", "truncated", "exit_code")
    CONTENT_FIELD_NUMBER: _ClassVar[int]
    TRUNCATED_FIELD_NUMBER: _ClassVar[int]
    EXIT_CODE_FIELD_NUMBER: _ClassVar[int]
    content: str
    truncated: bool
    exit_code: int
    def __init__(self, content: _Optional[str] = ..., truncated: _Optional[bool] = ..., exit_code: _Optional[int] = ...) -> None: ...

class PermissionRequest(_message.Message):
    __slots__ = ("tool", "scope", "reason", "confidence")
    TOOL_FIELD_NUMBER: _ClassVar[int]
    SCOPE_FIELD_NUMBER: _ClassVar[int]
    REASON_FIELD_NUMBER: _ClassVar[int]
    CONFIDENCE_FIELD_NUMBER: _ClassVar[int]
    tool: str
    scope: str
    reason: str
    confidence: float
    def __init__(self, tool: _Optional[str] = ..., scope: _Optional[str] = ..., reason: _Optional[str] = ..., confidence: _Optional[float] = ...) -> None: ...

class PermissionDecision(_message.Message):
    __slots__ = ("decision", "operator_note")
    class Decision(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
        __slots__ = ()
        DECISION_UNSPECIFIED: _ClassVar[PermissionDecision.Decision]
        ALLOW: _ClassVar[PermissionDecision.Decision]
        DENY: _ClassVar[PermissionDecision.Decision]
    DECISION_UNSPECIFIED: PermissionDecision.Decision
    ALLOW: PermissionDecision.Decision
    DENY: PermissionDecision.Decision
    DECISION_FIELD_NUMBER: _ClassVar[int]
    OPERATOR_NOTE_FIELD_NUMBER: _ClassVar[int]
    decision: PermissionDecision.Decision
    operator_note: str
    def __init__(self, decision: _Optional[_Union[PermissionDecision.Decision, str]] = ..., operator_note: _Optional[str] = ...) -> None: ...
