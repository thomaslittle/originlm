"""Repository release hygiene and documentation links, without reading user files."""
import json,pathlib,re,subprocess,sys
ROOT=pathlib.Path(__file__).resolve().parents[1]
def main():
    tracked=subprocess.check_output(['git','ls-files','-z','--cached','--others','--exclude-standard'],cwd=ROOT).decode().split('\0');errors=[]
    for name in sorted(set(filter(None,tracked))):
        path=ROOT/name
        if not path.is_file():continue
        if name.startswith(('crates/','packages/','scripts/')) and path.suffix in ('.rs','.ts','.tsx','.mjs','.py') and '/tests/' not in name:
            text=path.read_text(encoding='utf-8')
            for pattern in (r'\bunimplemented!\(',r'\btodo!\(',r'^\s*raise NotImplementedError',r'NotImplemented,\s*"[^"\n]*tests'):
                if re.search(pattern,text,re.MULTILINE):errors.append(name+': unfinished implementation stub')
        if path.suffix=='.md' and name!='THIRD_PARTY_NOTICES.md' and not name.startswith('packages/'):
            text=path.read_text(encoding='utf-8')
            for link in re.findall(r'\[[^\]]*\]\(([^ )]+)(?: +[^)]*)?\)',text):
                if ':' in link or link.startswith(('#','/')):continue
                target=link.split('#')[0].split('?')[0]
                if target and not (path.parent/target).exists():errors.append(name+': broken local link '+target)
    for name in ('sdk-typescript','selector-core','react','ui-react'):
        d=json.loads((ROOT/'packages'/name/'package.json').read_text(encoding='utf-8'))
        if d.get('private')is not True:errors.append(name+': private distribution guard missing')
        if not (ROOT/'packages'/name/'LICENSE').is_file():errors.append(name+': license missing')
    if errors:print('\n'.join(errors));return 1
    print('PASS project hygiene: no implementation stubs, local Markdown links, package privacy and notices');return 0
if __name__=='__main__':raise SystemExit(main())
