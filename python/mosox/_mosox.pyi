import pyarrow as pa

__version__: str

class MosoxError(Exception): ...

class Model:
    def __init__(self, text: str) -> None: ...
    @staticmethod
    def from_file(path: str) -> Model: ...
    def to_mps(self, data: dict[str, pa.Table], prune: bool, check: bool) -> str: ...
    def solve(
        self,
        data: dict[str, pa.Table],
        prune: bool,
        check: bool,
        options: list[tuple[str, str]],
        verbose: bool,
    ) -> tuple[float, list[tuple[str, pa.Table]], list[tuple[str, pa.Table]]]: ...
