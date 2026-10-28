"""Fail on generated public contract drift; never edit files during verification."""
from generate_contracts import main
import sys
import subprocess
from pathlib import Path
if __name__ == '__main__':
    sys.argv = [sys.argv[0], '--check']
    status = main()
    if not status:
        status = subprocess.call([sys.executable, str(Path(__file__).with_name("generate_brand_tokens.py")), "--check"])
    raise SystemExit(status)
