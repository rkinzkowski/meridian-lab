"""Collect installed package notices; this inventory is broader than the frozen bundle."""
import argparse
import importlib.metadata
import json
from pathlib import Path
import re
import shutil


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    args.output.mkdir(parents=True,exist_ok=True)
    inventory=[]
    for distribution in importlib.metadata.distributions():
        name=distribution.metadata.get('Name','unnamed')
        folder=re.sub(r'[^A-Za-z0-9._-]','_',name+'-'+distribution.version)
        found=[]
        for path in distribution.files or []:
            if not any(part.endswith('.dist-info') for part in path.parts):
                continue
            if not any(word in str(path).lower() for word in ('license','copying','notice')):
                continue
            source=Path(distribution.locate_file(path))
            if source.is_file() and source.stat().st_size<4*1024*1024:
                dest=args.output/folder/Path(*path.parts[1:])
                dest.parent.mkdir(parents=True,exist_ok=True)
                shutil.copyfile(source,dest)
                found.append(str(dest.relative_to(args.output)).replace('\\','/'))
        inventory.append({'name':name,'version':distribution.version,'license':distribution.metadata.get('License-Expression') or distribution.metadata.get('License'),'noticeFiles':found})
    (args.output/'inventory.json').write_text(json.dumps({'scope':'Installed Python build environment; includes packages excluded from frozen runtime. Not a complete native/frontend SBOM.','packages':inventory},indent=2)+'\n',encoding='utf-8')
    print(f'Collected available notices for {len(inventory)} installed distributions.')


if __name__=='__main__':
    main()
