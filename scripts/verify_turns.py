import json

with open('/Users/Gress/.deepseek/sessions/038d91e9-73d7-4ae9-8037-4371e115d381.json') as f:
    s = json.load(f)

msgs = s['messages']
turns = []
ct = None

for m in msgs:
    r = m.get('role', '')
    c = m.get('content', '')
    blks = c if isinstance(c, list) else [{'type': 'text', 'text': str(c)}]
    has_tr = any(b.get('type') == 'tool_result' for b in blks)

    if r == 'user' and not has_tr:
        if ct:
            turns.append(ct)
        ut = ''.join(b.get('text', '') for b in blks if b.get('type') == 'text')
        ct = {'ui': ut[:80], 'mc': 0, 'tc': 0, 'trc': 0, 'th': 0}
    elif r == 'assistant' and ct:
        ct['mc'] += 1
        ct['tc'] += sum(1 for b in blks if b.get('type') == 'tool_use')
        ct['th'] += sum(1 for b in blks if b.get('type') == 'thinking')
    elif r == 'user' and has_tr and ct:
        ct['mc'] += 1
        ct['trc'] += sum(1 for b in blks if b.get('type') == 'tool_result')

if ct:
    turns.append(ct)

print(f'分组: {len(turns)} turns')
for i, t in enumerate(turns):
    print(f'  Turn {i+1}: ui={t["ui"][:50]}... msgs={t["mc"]} th={t["th"]} tu={t["tc"]} tr={t["trc"]}')
