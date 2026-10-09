"""Shared-database fixture creation is never an implicit smoke-test action."""
import os

def require_fixture_opt_in():
    if os.environ.get('HELLODOCTOR_ALLOW_SHARED_DEV_FIXTURES') != 'true':
        raise SystemExit(
            'Refusing shared development fixture mutations. Prefer isolated Rust '
            'api_tests. Set HELLODOCTOR_ALLOW_SHARED_DEV_FIXTURES=true only when '
            'creating persistent synthetic accounts/slots is explicitly approved.')
