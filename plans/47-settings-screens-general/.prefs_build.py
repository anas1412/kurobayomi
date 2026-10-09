import re, os
R="/home/blackbox/aniyomi-src/"
FILES=["app/src/main/java/eu/kanade/domain/base/BasePreferences.kt","app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt","app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt","app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt","app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt","core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt","core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkPreferences.kt","domain/src/main/java/tachiyomi/domain/backup/service/BackupPreferences.kt","domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt","domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt","domain/src/main/java/tachiyomi/domain/storage/service/StoragePreferences.kt"]
def args_of(src, start):
    # start points at '(' ; return inner text of balanced parens
    depth=0
    for i in range(start, len(src)):
        c=src[i]
        if c=='(': depth+=1
        elif c==')':
            depth-=1
            if depth==0: return src[start+1:i]
    return ""
def split_top(s):
    out=[];depth=0;cur=""
    for c in s:
        if c in "([{": depth+=1
        if c in ")]}": depth-=1
        if c=="," and depth==0:
            out.append(cur.strip()); cur=""
        else: cur+=c
    if cur.strip(): out.append(cur.strip())
    return out
rows=[]
for f in FILES:
    src=open(R+f,encoding="utf-8").read()
    consts=dict(re.findall(r'const val (\w+) = ("[^"]*"|\w+|0x[0-9A-Fa-f]+L?|-?\d+L?)', src))
    consts.update(dict(re.findall(r'private const val (\w+) = ("[^"]*")', src)))
    # class-level function names
    for m in re.finditer(r'preferenceStore\.(getBoolean|getInt|getLong|getString|getStringSet|getEnum|getObject)\(', src):
        a=args_of(src, m.end()-1)
        parts=split_top(a)
        if not parts: continue
        key=parts[0]
        # find enclosing fun name
        pre=src[:m.start()]
        fns=re.findall(r'fun (\w+)\(', pre)
        fname=fns[-1] if fns else "?"
        keyres=key
        if key.startswith('"'): keyres=key.strip('"')
        else:
            kk=key
            mm=re.match(r'Preference\.(appStateKey|privateKey)\((.*)\)$', key, re.S)
            if mm:
                inner=mm.group(2).strip()
                inner=consts.get(inner, inner).strip('"')
                prefix="__APP_STATE_" if mm.group(1)=="appStateKey" else "__PRIVATE_"
                keyres=prefix+inner
            else:
                keyres=consts.get(key, key).strip('"')
        default=", ".join(parts[1:]).replace("|","\\|").replace("\n"," ")
        default=re.sub(r"\s+"," ",default)
        rows.append((os.path.basename(f), fname, m.group(1), keyres, default))
    # string-interpolated keys
    for m in re.finditer(r'preferenceStore\.(getBoolean|getInt|getLong|getString|getStringSet|getEnum|getObject)\(\s*Preference\.(privateKey|appStateKey)\(\s*"([^"]*)"', src):
        pass
print("| File | Function | Store call | Key (stored name) | Default expression |")
print("|---|---|---|---|---|")
for r in rows:
    print(f"| `{r[0]}` | `{r[1]}()` | `{r[2]}` | `{r[3]}` | `{r[4]}` |")
print(f"\n<!-- rows={len(rows)} -->")
