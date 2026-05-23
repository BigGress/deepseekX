import json, os, glob
sessions = sorted(glob.glob(os.path.expanduser('~/.deepseek/sessions/*.json')), key=os.path.getmtime, reverse=True)
for s in sessions[:8]:
    with open(s) as f:
        data = json.load(f)
    msgs = data.get('messages', [])
    if msgs:
        c = msgs[-1].get('content', '')
        fmt = 'str' if isinstance(c, str) else f'arr[{len(c)}]'
        role = msgs[-1].get('role', '?')
        title = data.get('metadata', {}).get('title', '?')[:30]
        print(f'{os.path.basename(s)[:8]}: {len(msgs)} msgs, last={role}/{fmt}, title={title}')
