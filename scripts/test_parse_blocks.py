"""parse_file_blocks 单元测试"""
text = '''
```go file:main.go
package main
import "fmt"
func main() { fmt.Println("hello") }
```
中间文字。

```text file:go.mod
module myproject
go 1.21
```
普通代码块:
```python
print("hello")
```
嵌套: ```file:cmd/server/main.go
package server
```
危险: ```file:../../../etc/passwd
evil
```
绝对: ```file:/etc/hosts
evil
```
'''

lines = text.split('\n')
results = []
i = 0
while i < len(lines):
    line = lines[i].strip()
    if '```' in line and 'file:' in line:
        pos = line.find('file:')
        fp = line[pos+5:].strip()
        if fp and '..' not in fp and not fp.startswith('/'):
            i += 1
            cl = []
            while i < len(lines) and not lines[i].strip().startswith('```'):
                cl.append(lines[i]); i += 1
            code = '\n'.join(cl)
            if code: results.append((fp, code))
    i += 1

assert len(results) == 3, f"want 3, got {len(results)}"
assert results[0][0] == "main.go"
assert results[1][0] == "go.mod"
assert results[2][0] == "cmd/server/main.go"
print(f"OK: {len(results)} files parsed, all assertions passed")
