"""Freeze only imported production modules into a branded, self-contained runtime."""
import argparse
import importlib.metadata
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, default=ROOT/'apps/desktop/src-tauri/runtime')
    parser.add_argument('--work', type=Path, default=ROOT/'work/freeze')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    args.work.mkdir(parents=True, exist_ok=True)
    command = [sys.executable,'-m','PyInstaller','--noconfirm','--clean','--onedir','--noupx','--name','meridian-runtime','--distpath',str(args.output),'--workpath',str(args.work/'build'),'--specpath',str(args.work),'--paths',str(ROOT/'python-runtime'),'--copy-metadata','pymobiledevice3','--collect-data','pymobiledevice3']
    for module in ['IPython','xonsh','tkinter','pytest','pip','setuptools','matplotlib','av','pymobiledevice3.cli']:
        command += ['--exclude-module',module]
    # tunnel_service imports the vendor's Windows adapter module even when the
    # selected transport uses a userspace stack. Its DLL is loaded via ctypes
    # and cannot be discovered from the import graph alone. No adapter is created.
    if sys.platform == 'win32':
        command += ['--collect-binaries', 'pytun_pmd3', '--collect-data', 'pytun_pmd3']
    command.append(str(ROOT/'python-runtime/runtime_entry.py'))
    subprocess.run(command,check=True)
    # Dependency inventory describes the build environment; frozen-module inventory
    # remains PyInstaller Analysis-00.toc. Do not mislabel this as a release SBOM.
    inventory=[]
    for dist in importlib.metadata.distributions():
        inventory.append({'name':dist.metadata.get('Name'),'version':dist.version,'license':dist.metadata.get('License-Expression') or dist.metadata.get('License')})
    (args.output/'build-dependency-inventory.json').write_text(json.dumps(inventory,indent=2)+'\n',encoding='utf-8')


if __name__=='__main__':
    main()
