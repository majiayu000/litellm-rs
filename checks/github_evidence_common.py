"""Shared errors for read-only GitHub evidence adapters."""


class EvidenceError(ValueError):
    """Raised when GitHub evidence cannot be collected or normalized."""
