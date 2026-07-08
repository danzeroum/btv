from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class HealthRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class HealthResponse(_message.Message):
    __slots__ = ("ready", "version")
    READY_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    ready: bool
    version: str
    def __init__(self, ready: _Optional[bool] = ..., version: _Optional[str] = ...) -> None: ...

class LintRequest(_message.Message):
    __slots__ = ("prompt",)
    PROMPT_FIELD_NUMBER: _ClassVar[int]
    prompt: str
    def __init__(self, prompt: _Optional[str] = ...) -> None: ...

class LintIssue(_message.Message):
    __slots__ = ("rule", "message")
    RULE_FIELD_NUMBER: _ClassVar[int]
    MESSAGE_FIELD_NUMBER: _ClassVar[int]
    rule: str
    message: str
    def __init__(self, rule: _Optional[str] = ..., message: _Optional[str] = ...) -> None: ...

class LintReport(_message.Message):
    __slots__ = ("score", "grade", "issues")
    SCORE_FIELD_NUMBER: _ClassVar[int]
    GRADE_FIELD_NUMBER: _ClassVar[int]
    ISSUES_FIELD_NUMBER: _ClassVar[int]
    score: float
    grade: str
    issues: _containers.RepeatedCompositeFieldContainer[LintIssue]
    def __init__(self, score: _Optional[float] = ..., grade: _Optional[str] = ..., issues: _Optional[_Iterable[_Union[LintIssue, _Mapping]]] = ...) -> None: ...

class RenderRequest(_message.Message):
    __slots__ = ("generator", "fields")
    class FieldsEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: str
        def __init__(self, key: _Optional[str] = ..., value: _Optional[str] = ...) -> None: ...
    GENERATOR_FIELD_NUMBER: _ClassVar[int]
    FIELDS_FIELD_NUMBER: _ClassVar[int]
    generator: str
    fields: _containers.ScalarMap[str, str]
    def __init__(self, generator: _Optional[str] = ..., fields: _Optional[_Mapping[str, str]] = ...) -> None: ...

class RenderResponse(_message.Message):
    __slots__ = ("prompt",)
    PROMPT_FIELD_NUMBER: _ClassVar[int]
    prompt: str
    def __init__(self, prompt: _Optional[str] = ...) -> None: ...

class GeneratorField(_message.Message):
    __slots__ = ("name", "label", "required", "placeholder")
    NAME_FIELD_NUMBER: _ClassVar[int]
    LABEL_FIELD_NUMBER: _ClassVar[int]
    REQUIRED_FIELD_NUMBER: _ClassVar[int]
    PLACEHOLDER_FIELD_NUMBER: _ClassVar[int]
    name: str
    label: str
    required: bool
    placeholder: str
    def __init__(self, name: _Optional[str] = ..., label: _Optional[str] = ..., required: _Optional[bool] = ..., placeholder: _Optional[str] = ...) -> None: ...

class GeneratorInfo(_message.Message):
    __slots__ = ("name", "category", "fields")
    NAME_FIELD_NUMBER: _ClassVar[int]
    CATEGORY_FIELD_NUMBER: _ClassVar[int]
    FIELDS_FIELD_NUMBER: _ClassVar[int]
    name: str
    category: str
    fields: _containers.RepeatedCompositeFieldContainer[GeneratorField]
    def __init__(self, name: _Optional[str] = ..., category: _Optional[str] = ..., fields: _Optional[_Iterable[_Union[GeneratorField, _Mapping]]] = ...) -> None: ...

class ListGeneratorsRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class ListGeneratorsResponse(_message.Message):
    __slots__ = ("generators",)
    GENERATORS_FIELD_NUMBER: _ClassVar[int]
    generators: _containers.RepeatedCompositeFieldContainer[GeneratorInfo]
    def __init__(self, generators: _Optional[_Iterable[_Union[GeneratorInfo, _Mapping]]] = ...) -> None: ...
