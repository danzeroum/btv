from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class LlmRequest(_message.Message):
    __slots__ = ("model", "messages_json", "temperature", "max_tokens", "requester")
    MODEL_FIELD_NUMBER: _ClassVar[int]
    MESSAGES_JSON_FIELD_NUMBER: _ClassVar[int]
    TEMPERATURE_FIELD_NUMBER: _ClassVar[int]
    MAX_TOKENS_FIELD_NUMBER: _ClassVar[int]
    REQUESTER_FIELD_NUMBER: _ClassVar[int]
    model: str
    messages_json: str
    temperature: float
    max_tokens: int
    requester: str
    def __init__(self, model: _Optional[str] = ..., messages_json: _Optional[str] = ..., temperature: _Optional[float] = ..., max_tokens: _Optional[int] = ..., requester: _Optional[str] = ...) -> None: ...

class LlmChunk(_message.Message):
    __slots__ = ("text_delta", "usage", "error")
    TEXT_DELTA_FIELD_NUMBER: _ClassVar[int]
    USAGE_FIELD_NUMBER: _ClassVar[int]
    ERROR_FIELD_NUMBER: _ClassVar[int]
    text_delta: str
    usage: Usage
    error: str
    def __init__(self, text_delta: _Optional[str] = ..., usage: _Optional[_Union[Usage, _Mapping]] = ..., error: _Optional[str] = ...) -> None: ...

class Usage(_message.Message):
    __slots__ = ("input_tokens", "output_tokens", "cache_hit", "provider")
    INPUT_TOKENS_FIELD_NUMBER: _ClassVar[int]
    OUTPUT_TOKENS_FIELD_NUMBER: _ClassVar[int]
    CACHE_HIT_FIELD_NUMBER: _ClassVar[int]
    PROVIDER_FIELD_NUMBER: _ClassVar[int]
    input_tokens: int
    output_tokens: int
    cache_hit: bool
    provider: str
    def __init__(self, input_tokens: _Optional[int] = ..., output_tokens: _Optional[int] = ..., cache_hit: _Optional[bool] = ..., provider: _Optional[str] = ...) -> None: ...
