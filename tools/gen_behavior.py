"""Generate src/behavior/classes.rs and src/behavior/enums.rs.

usage: python -I gen_behavior.py <census-sekiro.json> <census-elden.json> <hklib2018.json> <out dir>

Inputs (the two censuses are JSON made by a pass over the type table of every behaviour tagfile of one install; they are
not distributed):
  census-<game>.json: {"layouts": {class name: [layout]}} with one layout for each class name, as
    {"parent": name or null, "format": int, "size": int, "version": int or null,
     "fields": [[member name, byte offset, member flags, type name with its arguments expanded], ...]}
    (a type name spells its template arguments in angle brackets: hkArray<hkReal,hkContainerHeapAllocator>).
  hklib2018.json: {"enums": {Outer::Inner: {"values": [[member name, int value], ...]}}}, made by hklib_model.py from
    HKLib's hk2018 classes (MIT).
A class gets one view with an accessor for each serialised member of its layout (its own and inherited ones); a member that
only one game's layout has returns an Option.
"""
import json, re, sys, os

sek, eld, hk, out = sys.argv[1:5]
S = json.load(open(sek)); E = json.load(open(eld)); H = json.load(open(hk))
L = {'sekiro': {k: v[0] for k, v in S['layouts'].items()}, 'elden': {k: v[0] for k, v in E['layouts'].items()}}
GAMES = ['elden', 'sekiro']
GAME_NAME = {'elden': 'Elden Ring', 'sekiro': 'Sekiro'}
names = sorted(set(L['sekiro']) | set(L['elden']))
def layout(game, n): return L[game].get(n)
def is_class(n):
    l = layout('elden', n) or layout('sekiro', n)
    return ((l['format'] or 0) & 15) == 7
classes = [n for n in names if is_class(n)]
enums = [n for n in names if not is_class(n)]

# ---- names
def pascal(n):
    n = n[3:] if n.startswith('hkb') else n
    return ''.join(p[:1].upper() + p[1:] for p in n.split('::'))
def snake(n):
    s = re.sub(r'([a-z0-9])([A-Z])', r'\1_\2', n)
    s = re.sub(r'([A-Z]+)([A-Z][a-z])', r'\1_\2', s)
    return s.lower()
def const_name(n):
    return snake(n.replace('::', '_').replace('hkb', 'hkb_', 1) if n.startswith('hkb') else n.replace('::', '_')).upper().replace('__', '_')
KEYWORDS = {'type', 'loop', 'match', 'move', 'ref', 'mod', 'fn', 'in', 'as', 'box', 'do', 'final', 'macro', 'override', 'priv', 'try',
            'typeof', 'unsized', 'virtual', 'yield', 'where', 'while', 'use', 'static', 'struct', 'trait', 'true', 'false', 'unsafe',
            'impl', 'if', 'else', 'enum', 'extern', 'for', 'let', 'mut', 'pub', 'return', 'const', 'continue', 'break', 'async', 'await', 'dyn', 'abstract', 'become'}
RESERVED = {'raw', 'from_value', 'cast', 'clone', 'fmt', 'eq', 'hash', 'class'}
view_name = {n: pascal(n) for n in classes}
assert len(set(view_name.values())) == len(classes), 'view name collision'

# ---- type grammar:  Name<arg,arg>
def split_args(s):
    out = []; depth = 0; cur = ''
    for ch in s:
        if ch == '<': depth += 1
        if ch == '>': depth -= 1
        if ch == ',' and depth == 0:
            out.append(cur); cur = ''
        else: cur += ch
    if cur: out.append(cur)
    return out
def parse_type(s):
    i = s.find('<')
    if i < 0: return (s, [])
    assert s.endswith('>'), s
    return (s[:i], split_args(s[i + 1:-1]))

INTS = {'hkInt8': 'i8', 'hkInt16': 'i16', 'hkInt32': 'i32', 'int': 'i32'}
UINTS = {'hkUint8': 'u8', 'hkUint16': 'u16', 'hkUint32': 'u32', 'hkUlong': 'u64'}
POINTERS = ('T*', 'hkRefPtr', 'hkViewPtr')

# a pointee class with no registered subclass is read as its view
children = {}
for n in classes:
    p = (layout('elden', n) or layout('sekiro', n))['parent']
    children.setdefault(p, []).append(n)
def concrete(n):
    return n in view_name and not children.get(n)

def scalar_kind(t):
    base, args = parse_type(t)
    if t == 'hkReal': return 'real'
    if t in ('hkBool', 'bool'): return 'bool'
    if t in INTS: return 'int'
    if t in UINTS: return 'uint'
    return None

def accessor(t, v):
    """(rust type, expression of type Result<rust type> reading value expression `v`), or None for an unreadable type."""
    base, args = parse_type(t)
    if t == 'hkReal': return ('f32', f'real({v})')
    if t in ('hkBool', 'bool'): return ('bool', f'boolean({v})')
    if t in INTS: return (INTS[t], f'int::<{INTS[t]}>({v})')
    if t in UINTS: return (UINTS[t], f'uint::<{UINTS[t]}>({v})')
    if t == 'hkStringPtr': return ('String', f'{v}.string()')
    if t in ('hkVector4', 'hkQuaternion'): return ('[f32; 4]', f'{v}.vector::<4>()')
    if t == 'hkQsTransform': return ('[[f32; 4]; 3]', f'qs_transform({v})')
    if t == 'hkMatrix4': return ('Vec<f32>', f'reals({v})')
    if base in ('hkEnum', 'hkFlags'):
        return ('EnumValue', f'enumeration({v}, "{args[0]}")')
    if base in POINTERS:
        target = args[0]
        if concrete(target): return (f"Option<{view_name[target]}<'t, 'a>>", f'pointer_view::<{view_name[target]}>({v})')
        return ("Option<Object<'t, 'a>>", f'object({v})')
    if t == 'hkRefVariant': return ("Option<Object<'t, 'a>>", f'object({v})')
    if base == 'T[N]':
        el = args[0]
        k = scalar_kind(el)
        if k == 'real': return ('Vec<f32>', f'reals({v})')
        if k == 'int': return (f'Vec<{INTS[el]}>', f'ints::<{INTS[el]}>({v})')
        if k == 'uint': return (f'Vec<{UINTS[el]}>', f'uints::<{UINTS[el]}>({v})')
        return None
    if base == 'hkArray':
        el = args[0]
        k = scalar_kind(el)
        if k == 'real': return ('Vec<f32>', f'reals({v})')
        if k == 'int': return (f'Vec<{INTS[el]}>', f'ints::<{INTS[el]}>({v})')
        if k == 'uint': return (f'Vec<{UINTS[el]}>', f'uints::<{UINTS[el]}>({v})')
        if el == 'hkStringPtr': return ('Vec<String>', f'strings({v})')
        if el in ('hkVector4', 'hkQuaternion'): return ('Vec<[f32; 4]>', f'vectors({v})')
        if el == 'hkQsTransform': return ('Vec<[[f32; 4]; 3]>', f'qs_transforms({v})')
        if el == 'hkMatrix4': return ('Vec<Vec<f32>>', f'real_rows({v})')
        eb, ea = parse_type(el)
        if eb in POINTERS or el == 'hkRefVariant':
            if eb in POINTERS and concrete(ea[0]):
                return (f"Vec<Option<{view_name[ea[0]]}<'t, 'a>>>", f'pointer_views::<{view_name[ea[0]]}>({v})')
            return ("Vec<Option<Object<'t, 'a>>>", f'objects({v})')
        if el in view_name: return (f"Vec<{view_name[el]}<'t, 'a>>", f'elements::<{view_name[el]}>({v})')
        return None
    if t in view_name: return (f"{view_name[t]}<'t, 'a>", f'Ok({view_name[t]}::from_value({v}))')
    return None

# ---- effective members of each class: root-first through registered parents
def chain(game, n):
    out = []
    while n and layout(game, n):
        out.append(n); n = layout(game, n)['parent']
    return list(reversed(out))
def members(game, n):
    res = []
    for c in chain(game, n):
        for f in layout(game, c)['fields']:
            if f[2] & 1: continue
            res.append((f[0], f[3], c))
    return res

unreadable = []
class_src = []
for n in classes:
    present = [g for g in GAMES if layout(g, n)]
    per = {g: {m[0]: m for m in members(g, n)} for g in present}
    order = []
    for g in present:
        for m in members(g, n):
            if m[0] not in order: order.append(m[0])
    vn = view_name[n]
    snk = {}
    for m in order:
        s = snake(m)
        if s in KEYWORDS: s = 'r#' + s
        if s in RESERVED: s += '_member'
        assert s not in snk.values(), (n, m, s)
        snk[m] = s
    methods = []
    reads = []
    enum_reads = []
    for m in order:
        tys = {g: per[g][m][1] for g in present if m in per[g]}
        assert len(set(tys.values())) == 1, (n, m, tys)
        t = next(iter(tys.values()))
        owner = next(per[g][m][2] for g in present if m in per[g])
        a = accessor(t, f'self.0.field("{m}")?')
        if a is None:
            unreadable.append((n, m, t)); continue
        rtype, expr = a
        optional = len(tys) < len(present)
        if parse_type(t)[0] in ('hkEnum', 'hkFlags'):
            if optional:
                enum_reads.append(f"""        if let Some(value) = self.{snk[m]}()? {{
            total = total.saturating_add(1);
            if !value.is_named() {{
                unnamed = unnamed.saturating_add(1);
            }}
        }}""")
            else:
                enum_reads.append(f"""        let value = self.{snk[m]}()?;
        total = total.saturating_add(1);
        if !value.is_named() {{
            unnamed = unnamed.saturating_add(1);
        }}""")
        if optional:
            reads.append(f"        if self.{snk[m]}()?.is_some() {{\n            count = count.saturating_add(1);\n        }}")
        else:
            reads.append(f"        self.{snk[m]}()?;\n        count = count.saturating_add(1);")
        doc = f"/// Member `{m}` of `{owner}`, a `{t}`."
        if optional:
            only = ' and '.join(GAME_NAME[g] for g in present if m in per[g])
            doc += f"\n    /// Only {only}'s layout of the class has it: `None` for the other game's."
            methods.append(f"""    {doc}
    pub fn {snk[m]}(self) -> Result<Option<{rtype}>> {{
        if !self.0.has_field("{m}") {{
            return Ok(None);
        }}
        {expr}.map(Some)
    }}""")
        else:
            methods.append(f"""    {doc}
    pub fn {snk[m]}(self) -> Result<{rtype}> {{
        {expr}
    }}""")
    parent = (layout('elden', n) or layout('sekiro', n))['parent']
    where = '' if len(present) == 2 else f' Only {GAME_NAME[present[0]]}\'s type tables have this class.'
    if reads:
        reader = ("    /// Read every member through its accessor and count them: the members this file's class has.\n"
                  "    pub fn read_members(self) -> Result<usize> {\n        let mut count = 0usize;\n" + "\n".join(reads)
                  + "\n        Ok(count)\n    }")
    else:
        reader = "    /// The class has no members to read.\n    pub fn read_members(self) -> Result<usize> {\n        Ok(0)\n    }"
    if enum_reads:
        enum_reader = ("    /// The enumeration and flag-set members of the object and how many hold a value the table does not name:\n"
                       "    /// `(members, unnamed)`.\n    pub fn enum_members(self) -> Result<(usize, usize)> {\n"
                       "        let mut total = 0usize;\n        let mut unnamed = 0usize;\n"
                       + "\n".join(enum_reads) + "\n        Ok((total, unnamed))\n    }")
    else:
        enum_reader = ("    /// The class has no enumeration or flag-set members.\n"
                       "    pub fn enum_members(self) -> Result<(usize, usize)> {\n        Ok((0, 0))\n    }")
    body = "\n\n".join(methods + [reader, enum_reader])
    impl_block = f"\nimpl<'t, 'a> {vn}<'t, 'a> {{\n{body}\n}}\n"
    class_src.append(f"""
/// A view of a `{n}` object{'' if not parent else f' (derives from `{parent}`)'}.{where}
#[derive(Clone, Copy, Debug)]
pub struct {vn}<'t, 'a>(Value<'t, 'a>);

impl<'t, 'a> View<'t, 'a> for {vn}<'t, 'a> {{
    const CLASS: &'static str = "{n}";
    fn from_value(value: Value<'t, 'a>) -> Self {{
        Self(value)
    }}
    fn raw(self) -> Value<'t, 'a> {{
        self.0
    }}
}}
{impl_block}""")

# ---- Class enum and Typed enum
class_variants = [view_name[n] for n in classes]
class_enum = ["""
/// The classes of the behaviour tagfiles of Sekiro and Elden Ring: every class whose name starts with `hkb` or `Custom`
/// that a type table of either game holds, nested structures included.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Class {"""]
for n in classes:
    class_enum.append(f"    /// `{n}`.\n    {view_name[n]},")
class_enum.append("}\n\nimpl Class {\n    /// Every class, in name order.\n    pub const ALL: &'static [Class] = &[")
for n in classes: class_enum.append(f"        Class::{view_name[n]},")
class_enum.append("    ];\n\n    /// The class a type name (as a tagfile spells it, `Outer::Inner` for a nested one) names.\n    pub fn from_name(name: &str) -> Option<Class> {\n        match name {")
for n in classes: class_enum.append(f'            "{n}" => Some(Class::{view_name[n]}),')
class_enum.append("            _ => None,\n        }\n    }\n\n    /// The name as a tagfile spells it.\n    pub fn name(self) -> &'static str {\n        match self {")
for n in classes: class_enum.append(f'            Class::{view_name[n]} => "{n}",')
class_enum.append("        }\n    }\n\n    /// The class this one derives from, when that is one of these classes.\n    pub fn parent(self) -> Option<Class> {\n        match self {")
for n in classes:
    p = (layout('elden', n) or layout('sekiro', n))['parent']
    class_enum.append(f"            Class::{view_name[n]} => " + (f"Some(Class::{view_name[p]})," if p in view_name else "None,"))
class_enum.append("        }\n    }\n}\n")

typed = ["""
/// An object read as the view of its own class: one variant for each class of [`Class`].
#[derive(Clone, Copy, Debug)]
pub enum Typed<'t, 'a> {"""]
for n in classes: typed.append(f"    /// A `{n}`.\n    {view_name[n]}({view_name[n]}<'t, 'a>),")
typed.append("}\n\nimpl<'t, 'a> Typed<'t, 'a> {\n    /// The view of `value` for the class its type names; `None` for a class that is not one of [`Class`].\n    pub fn of(value: Value<'t, 'a>) -> Option<Self> {\n        match Class::from_name(value.type_name())? {")
for n in classes: typed.append(f"            Class::{view_name[n]} => Some(Typed::{view_name[n]}({view_name[n]}::from_value(value))),")
typed.append("        }\n    }\n\n    /// Read every member of the object through its view's `read_members`.\n    pub fn read_members(self) -> Result<usize> {\n        match self {")
for n in classes: typed.append(f"            Typed::{view_name[n]}(v) => v.read_members(),")
typed.append("        }\n    }\n\n    /// The enumeration and flag-set members of the object and how many hold a value the table does not name.\n    pub fn enum_members(self) -> Result<(usize, usize)> {\n        match self {")
for n in classes: typed.append(f"            Typed::{view_name[n]}(v) => v.enum_members(),")
typed.append("        }\n    }\n\n    /// The class of the variant.\n    pub fn class(&self) -> Class {\n        match self {")
for n in classes: typed.append(f"            Typed::{view_name[n]}(_) => Class::{view_name[n]},")
typed.append("        }\n    }\n}\n")

header = "// Generated by gen_behavior.py from the type tables of every behaviour tagfile of Sekiro and Elden Ring and HKLib's enum\n// member names (MIT). Do not edit by hand: regenerate.\n"
body_text = ''.join(class_enum) + ''.join(typed) + ''.join(class_src)
HELPERS = ['boolean', 'elements', 'enumeration', 'int', 'ints', 'object', 'objects', 'pointer_view', 'pointer_views',
           'qs_transform', 'qs_transforms', 'real', 'real_rows', 'reals', 'strings', 'uint', 'uints', 'vectors']
used = [h for h in HELPERS if re.search(r'(?<![_a-z])' + h + r'(::<[^>]*>)?\(', body_text)]
imports = ', '.join(['EnumValue', 'Object', 'View'] + used)
classes_rs = header + f"""
//! Typed views of the classes of the behaviour tagfiles (see the [parent module](super)): one view for each class, an accessor
//! for each of its serialised members.

use super::{{{imports}}};
use crate::{{Result, Value}};
""" + body_text
os.makedirs(out, exist_ok=True)
open(os.path.join(out, 'classes.rs'), 'w', encoding='utf-8', newline='\n').write(classes_rs)

# ---- enums
en_src = [header + """
//! The enumerations and flag sets the behaviour classes use, with the member names HKLib's hk2018 classes give them.
//! A value the file stores is shown by [`EnumInfo::name_of`] or [`EnumInfo::flag_names`]; the files hold only values these
//! tables name (see the module documentation for the census).
use super::EnumInfo;
"""]
refs = {}
for g in GAMES:
    for n in classes:
        l = layout(g, n)
        if not l: continue
        for f in l['fields']:
            if f[2] & 1: continue
            b, a = parse_type(f[3])
            if b in ('hkEnum', 'hkFlags'): refs[a[0]] = b == 'hkFlags'
for n in enums:
    h = H['enums'].get(n)
    vals = h['values'] if h else []
    for nm, v in vals: assert isinstance(v, int), (n, nm, v)
    flags = refs.get(n, False)
    cn = const_name(n)
    members = ', '.join(f'("{nm}", {v})' for nm, v in vals)
    en_src.append(f"""
/// `{n}`{'' if h else ' (not in HKLib: no member names)'}.
pub static {cn}: EnumInfo = EnumInfo {{
    name: "{n}",
    flags: {str(flags).lower()},
    values: &[{members}],
}};
""")
en_src.append("\n/// The table of the enumeration or flag set a type name (as a tagfile spells it) names.\npub fn enum_info(name: &str) -> Option<&'static EnumInfo> {\n    match name {")
for n in enums: en_src.append(f'        "{n}" => Some(&{const_name(n)}),')
en_src.append("        _ => None,\n    }\n}\n\n/// Every table, in name order.\npub static ALL: &[&EnumInfo] = &[")
for n in enums: en_src.append(f"    &{const_name(n)},")
en_src.append("];\n")
open(os.path.join(out, 'enums.rs'), 'w', encoding='utf-8', newline='\n').write(''.join(en_src))
print('classes', len(classes), 'enums', len(enums), 'unreadable members', unreadable)
