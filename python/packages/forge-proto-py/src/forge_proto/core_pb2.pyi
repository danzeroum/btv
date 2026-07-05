import llm_pb2 as _llm_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable
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

class LedgerAppend(_message.Message):
    __slots__ = ("kind", "actor", "payload_json", "fake_marker")
    KIND_FIELD_NUMBER: _ClassVar[int]
    ACTOR_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_JSON_FIELD_NUMBER: _ClassVar[int]
    FAKE_MARKER_FIELD_NUMBER: _ClassVar[int]
    kind: str
    actor: str
    payload_json: str
    fake_marker: str
    def __init__(self, kind: _Optional[str] = ..., actor: _Optional[str] = ..., payload_json: _Optional[str] = ..., fake_marker: _Optional[str] = ...) -> None: ...

class LedgerAck(_message.Message):
    __slots__ = ("seq", "entry_hash")
    SEQ_FIELD_NUMBER: _ClassVar[int]
    ENTRY_HASH_FIELD_NUMBER: _ClassVar[int]
    seq: int
    entry_hash: str
    def __init__(self, seq: _Optional[int] = ..., entry_hash: _Optional[str] = ...) -> None: ...

class RecallRequest(_message.Message):
    __slots__ = ("agent", "query", "limit")
    AGENT_FIELD_NUMBER: _ClassVar[int]
    QUERY_FIELD_NUMBER: _ClassVar[int]
    LIMIT_FIELD_NUMBER: _ClassVar[int]
    agent: str
    query: str
    limit: int
    def __init__(self, agent: _Optional[str] = ..., query: _Optional[str] = ..., limit: _Optional[int] = ...) -> None: ...

class RecallResponse(_message.Message):
    __slots__ = ("memories_json",)
    MEMORIES_JSON_FIELD_NUMBER: _ClassVar[int]
    memories_json: _containers.RepeatedScalarFieldContainer[str]
    def __init__(self, memories_json: _Optional[_Iterable[str]] = ...) -> None: ...

class RememberRequest(_message.Message):
    __slots__ = ("agent", "memory_json")
    AGENT_FIELD_NUMBER: _ClassVar[int]
    MEMORY_JSON_FIELD_NUMBER: _ClassVar[int]
    agent: str
    memory_json: str
    def __init__(self, agent: _Optional[str] = ..., memory_json: _Optional[str] = ...) -> None: ...

class RememberAck(_message.Message):
    __slots__ = ("stored",)
    STORED_FIELD_NUMBER: _ClassVar[int]
    stored: bool
    def __init__(self, stored: _Optional[bool] = ...) -> None: ...

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
