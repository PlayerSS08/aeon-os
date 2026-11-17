import re,sys
src=open('reg.h',encoding='utf-8',errors='replace').read()+"\n"+open('mac.h',encoding='utf-8',errors='replace').read()
defs={}
for m in re.finditer(r'^#define\s+([A-Za-z_][A-Za-z0-9_]*)\s+(.+?)\s*(?:/\*.*)?$',src,re.M):
    defs.setdefault(m.group(1),m.group(2).strip())
# enum-style "NAME = 0x..," entries in mac.h (xtal si offsets)
for m in re.finditer(r'^\s+([A-Z][A-Z0-9_]+)\s*=\s*(0x[0-9A-Fa-f]+|\d+),',src,re.M):
    defs.setdefault(m.group(1),m.group(2))
def ev(expr,seen=()):
    e=expr
    e=re.sub(r'BIT\((\w+)\)',lambda m:f'(1<<({ev(m.group(1),seen)}))',e)
    e=re.sub(r'GENMASK\(([^,]+),([^)]+)\)',lambda m:f'(((1<<(({ev(m.group(1),seen)})+1))-1)&~((1<<({ev(m.group(2),seen)}))-1))',e)
    def name(m):
        n=m.group(0)
        if re.fullmatch(r'0[xX][0-9a-fA-F]+',n) or n.isdigit(): return n
        if n in seen: raise ValueError('loop '+n)
        if n in defs: return '('+ev(defs[n],seen+(n,))+')'
        raise KeyError(n)
    e=re.sub(r'\b[A-Za-z_]\w*\b',name,e)
    e=re.sub(r'(\d)[uUlL]+\b',r'\1',e)
    return e
names=sys.argv[1:]
out=[]
for n in names:
    try:
        v=eval(ev(defs[n]))
    except Exception as ex:
        print('SKIP',n,ex,file=sys.stderr); continue
    out.append((n,v))
for n,v in out:
    print(f'pub const {n}: u32 = {v:#x};')
