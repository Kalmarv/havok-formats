"""A small reader of HKLib's hk2018 Autogen C# files (MIT; pinned ac40bc9): classes with their parent, members and nested enums
(with integer values), keyed by the tagfile's spelling `Outer::Inner`. Used as a cross-check and for enum member names."""
import re, os, glob, json, sys

TOKEN = re.compile(r'\b(class|enum)\s+(\w+)\s*(?::\s*([^\{]*?))?\s*\{|\{|\}|(public\s+[\w<>\?\[\], \.]+?\s+m_(\w+)\s*(?:=[^;]*)?;)|(\w+)\s*=\s*(-?\w+)\s*,?', re.S)

def parse_file(text):
    classes = {}; enums = {}
    stack = []   # (kind, name, fullname)
    pos = 0
    # tokenise by lines for robustness
    for line in text.splitlines():
        s = line.strip()
        m = re.match(r'public\s+(?:abstract\s+)?(?:partial\s+)?(class|enum|interface)\s+(\w+)(?:\s*:\s*(.*))?$', s)
        if m:
            kind, name, base = m.groups()
            outer = [n for k, n, *_ in stack if k in ('class', 'enum', 'interface')]
            full = '::'.join(outer + [name])
            stack.append((kind, name, full, 'pending'))
            if kind in ('class', 'interface'):
                parent = None
                if base:
                    first = base.split(',')[0].strip()
                    parent = None if first.startswith('IHavokObject') or first == '' else first
                classes[full] = dict(parent=parent, bases=[b.strip() for b in (base or '').split(',')], members=[])
            else:
                enums[full] = dict(storage=(base or 'int').strip(), values=[])
            continue
        if s == '{':
            if stack and stack[-1][3] == 'pending':
                stack[-1] = stack[-1][:3] + ('open',)
            else:
                stack.append(('block', '', '', 'open'))
            continue
        if s == '}':
            if stack: stack.pop()
            continue
        if stack and stack[-1][0] == 'enum':
            m = re.match(r'(\w+)\s*=\s*(-?\w+)\s*,?$', s)
            if m:
                v = m.group(2)
                enums[stack[-1][2]]['values'].append([m.group(1), int(v, 0) if re.match(r'-?(0x)?[0-9a-fA-F]+$', v) else v])
            continue
        if stack and stack[-1][0] in ('class', 'interface'):
            m = re.match(r'public\s+([\w<>\?\[\], \.]+?)\s+m_(\w+)\s*(?:=.*)?;', s)
            if m:
                classes[stack[-1][2]]['members'].append([m.group(2), m.group(1)])
    return classes, enums

def load(root):
    classes = {}; enums = {}
    for p in sorted(glob.glob(os.path.join(root, '**', '*.cs'), recursive=True)):
        t = open(p, encoding='utf-8-sig').read()
        c, e = parse_file(t)
        classes.update(c); enums.update(e)
    return classes, enums

if __name__ == '__main__':
    root = sys.argv[1]
    c, e = load(root)
    json.dump(dict(classes=c, enums=e), open(sys.argv[2], 'w'), indent=1)
    print(len(c), 'classes', len(e), 'enums')
