//! Language detection from content.
//!
//! A detection counts as correct when it names the expected language, with
//! allowances where the more general language is just as right: JSX may be
//! reported as JavaScript (the JavaScript lexer handles JSX too), SCSS without
//! any Sass only syntax (variables, mixins, `//` comments) as CSS, which has
//! nesting too, and plain text counts as correct when nothing is detected.
//! JSON with comments is only expected as JSONC when it has comments.

use colorfulcode::Language;
use std::path::Path;

/// The most used languages. Every one of their samples must be detected.
const POPULAR: &[&str] = &[
    "javascript", "typescript", "tsx", "jsx", "python", "java", "c", "cpp", "csharp", "go", "rust", "php", "ruby",
    "kotlin", "swift", "bash", "powershell", "sql", "html", "css", "scss", "json", "yaml", "toml", "markdown",
    "dockerfile", "makefile", "lua", "r", "dart", "scala", "haskell", "elixir", "perl", "xml", "vue", "svelte", "diff",
    "ini",
];

fn detected(code: &str) -> &'static str {
    Language::detect(code).map_or("none", |l| l.id())
}

fn is_correct(expected: &str, found: &str) -> bool {
    found == expected
        || (expected == "jsx" && found == "javascript")
        || (expected == "scss" && found == "css")
        || (expected == "text" && found == "none")
}

/// `(expected id, detected id)` for every sample file.
fn samples() -> Vec<(String, &'static str)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/samples");
    let mut results = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let id = name.split('.').next().unwrap().to_string();
        results.push((id, detected(&std::fs::read_to_string(&path).unwrap())));
    }
    results.sort();
    results
}

#[test]
fn popular_samples() {
    let wrong: Vec<String> = samples()
        .into_iter()
        .filter(|(id, found)| POPULAR.contains(&id.as_str()) && !is_correct(id, found))
        .map(|(id, found)| format!("{id} detected as {found}"))
        .collect();
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn all_samples() {
    let results = samples();
    let right = results.iter().filter(|(id, found)| is_correct(id, found)).count();
    assert!(right * 100 >= results.len() * 95, "only {right} of {} samples detected", results.len());
}

const SNIPPETS: &[(&str, &str)] = &[
    ("javascript", "const total = items.reduce((sum, x) => sum + x.price, 0);\nconsole.log(`Total: ${total}`);\n"),
    ("javascript", "function debounce(fn, ms) {\n  let timer;\n  return (...args) => {\n    clearTimeout(timer);\n    timer = setTimeout(() => fn(...args), ms);\n  };\n}\n"),
    ("typescript", "interface User {\n  id: number;\n  name: string;\n}\n\nexport function greet(user: User): string {\n  return `Hello ${user.name}`;\n}\n"),
    ("tsx", "export function Button({ label }: { label: string }) {\n  return <button className=\"btn\">{label}</button>;\n}\n"),
    ("jsx", "export default function App() {\n  const [count, setCount] = useState(0);\n  return <button onClick={() => setCount(count + 1)}>{count}</button>;\n}\n"),
    ("python", "def fib(n):\n    a, b = 0, 1\n    for _ in range(n):\n        a, b = b, a + b\n    return a\n"),
    ("python", "import os\n\nfor name in os.listdir('.'):\n    if name.endswith('.py'):\n        print(name)\n"),
    ("java", "public class Main {\n    public static void main(String[] args) {\n        System.out.println(\"Hello\");\n    }\n}\n"),
    ("c", "#include <stdio.h>\n\nint main(void) {\n    printf(\"hello\\n\");\n    return 0;\n}\n"),
    ("cpp", "#include <iostream>\n\nint main() {\n    std::cout << \"hello\" << std::endl;\n}\n"),
    ("cpp", "template <typename T>\nT max(T a, T b) {\n    return a > b ? a : b;\n}\n"),
    ("csharp", "using System;\n\nclass Program {\n    static void Main() {\n        Console.WriteLine(\"Hello\");\n    }\n}\n"),
    ("csharp", "public class Person\n{\n    public string Name { get; set; }\n    public int Age { get; init; }\n}\n"),
    ("go", "package main\n\nimport \"fmt\"\n\nfunc main() {\n\tfmt.Println(\"hello\")\n}\n"),
    ("go", "func (s *Server) Start() error {\n\tln, err := net.Listen(\"tcp\", s.addr)\n\tif err != nil {\n\t\treturn err\n\t}\n\treturn s.serve(ln)\n}\n"),
    ("rust", "fn main() {\n    let mut v = Vec::new();\n    v.push(1);\n    println!(\"{:?}\", v);\n}\n"),
    ("rust", "impl Display for Point {\n    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {\n        write!(f, \"({}, {})\", self.x, self.y)\n    }\n}\n"),
    ("php", "<?php\n$name = $_GET['name'] ?? 'world';\necho \"Hello, $name\";\n"),
    ("php", "function greet($name) {\n    return \"Hello \" . $name;\n}\necho greet($user->name);\n"),
    ("ruby", "class Dog\n  def initialize(name)\n    @name = name\n  end\n\n  def bark\n    puts \"#{@name} says woof\"\n  end\nend\n"),
    ("ruby", "[1, 2, 3].each do |n|\n  puts n * 2\nend\n"),
    ("kotlin", "fun main() {\n    val names = listOf(\"a\", \"b\")\n    names.forEach { println(it) }\n}\n"),
    ("kotlin", "data class User(val name: String, val age: Int)\n\nfun User.isAdult() = age >= 18\n"),
    ("swift", "import SwiftUI\n\nstruct ContentView: View {\n    var body: some View {\n        Text(\"Hello\")\n    }\n}\n"),
    ("swift", "func greet(_ name: String) -> String {\n    guard !name.isEmpty else { return \"Hi\" }\n    return \"Hello, \\(name)\"\n}\n"),
    ("bash", "for f in *.txt; do\n  echo \"$f\"\ndone\n"),
    ("bash", "if [ -z \"$1\" ]; then\n  echo \"usage: $0 name\" >&2\n  exit 1\nfi\n"),
    ("powershell", "Get-ChildItem -Path . -Filter *.log | ForEach-Object {\n    Write-Host $_.Name\n}\n"),
    ("powershell", "$files = Get-ChildItem C:\\Temp\nif ($files.Count -gt 10) {\n    Remove-Item $files[0]\n}\n"),
    ("sql", "SELECT id, name FROM users WHERE age > 18 ORDER BY name;\n"),
    ("sql", "CREATE TABLE orders (\n  id INTEGER PRIMARY KEY,\n  total DECIMAL(10, 2) NOT NULL\n);\n"),
    ("html", "<div class=\"card\">\n  <h1>Title</h1>\n  <p>Some text</p>\n</div>\n"),
    ("html", "<ul>\n  <li><a href=\"/\">Home</a></li>\n  <li><a href=\"/about\">About</a></li>\n</ul>\n"),
    ("css", ".card {\n  padding: 1rem;\n  border-radius: 8px;\n  background: #fff;\n}\n"),
    ("css", "body { margin: 0; font-family: sans-serif; }\na:hover { color: red; }\n"),
    ("scss", "$primary: #333;\n\n.nav {\n  color: $primary;\n  &:hover { color: red; }\n}\n"),
    ("json", "{\n  \"name\": \"demo\",\n  \"version\": \"1.0.0\",\n  \"private\": true\n}\n"),
    ("json", "[{\"id\": 1, \"tags\": [\"a\", \"b\"]}, {\"id\": 2, \"tags\": []}]\n"),
    ("yaml", "name: CI\non: [push]\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n"),
    ("yaml", "version: \"3\"\nservices:\n  web:\n    image: nginx\n    ports:\n      - \"80:80\"\n"),
    ("toml", "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n"),
    ("markdown", "# Title\n\nSome **bold** text and a [link](https://example.com).\n\n- one\n- two\n"),
    ("markdown", "## Usage\n\nRun the tool:\n\n```sh\nnpm install\n```\n"),
    ("dockerfile", "FROM node:20\nWORKDIR /app\nCOPY . .\nRUN npm ci\nCMD [\"node\", \"index.js\"]\n"),
    ("makefile", "all: build\n\nbuild:\n\tgo build -o bin/app ./cmd/app\n\n.PHONY: all build\n"),
    ("makefile", "CC = gcc\nCFLAGS = -O2\n\n%.o: %.c\n\t$(CC) $(CFLAGS) -c $< -o $@\n"),
    ("lua", "local function greet(name)\n  print(\"Hello \" .. name)\nend\n\nfor i = 1, 3 do greet(i) end\n"),
    ("lua", "local t = {}\nfor k, v in pairs(config) do\n  if v ~= nil then t[k] = v end\nend\nreturn t\n"),
    ("r", "library(ggplot2)\ndata <- read.csv(\"data.csv\")\nsummary(data$value)\n"),
    ("r", "square <- function(x) {\n  x^2\n}\nresult <- sapply(1:10, square)\n"),
    ("dart", "void main() {\n  final names = <String>['a', 'b'];\n  for (final n in names) {\n    print(n);\n  }\n}\n"),
    ("dart", "class Counter extends StatefulWidget {\n  const Counter({super.key});\n\n  @override\n  State<Counter> createState() => _CounterState();\n}\n"),
    ("scala", "object Main extends App {\n  val xs = List(1, 2, 3)\n  println(xs.map(_ * 2))\n}\n"),
    ("scala", "case class Point(x: Int, y: Int)\n\ndef dist(p: Point): Double = math.sqrt(p.x * p.x + p.y * p.y)\n"),
    ("haskell", "main :: IO ()\nmain = do\n  let xs = [1..10]\n  print (sum xs)\n"),
    ("haskell", "module Main where\n\nimport Data.List (sort)\n\nmedian :: [Int] -> Int\nmedian xs = sort xs !! (length xs `div` 2)\n"),
    ("elixir", "defmodule Greeter do\n  def hello(name) do\n    IO.puts(\"Hello #{name}\")\n  end\nend\n"),
    ("elixir", "[1, 2, 3]\n|> Enum.map(fn x -> x * 2 end)\n|> Enum.sum()\n"),
    ("perl", "use strict;\nuse warnings;\n\nmy @items = (1, 2, 3);\nforeach my $i (@items) {\n    print \"$i\\n\";\n}\n"),
    ("perl", "my %ages = (tom => 30);\nwhile (my ($k, $v) = each %ages) {\n    print \"$k: $v\\n\" if $v =~ /\\d+/;\n}\n"),
    ("xml", "<?xml version=\"1.0\"?>\n<config>\n  <entry key=\"a\">1</entry>\n</config>\n"),
    ("xml", "<project>\n  <modelVersion>4.0.0</modelVersion>\n  <groupId>com.example</groupId>\n</project>\n"),
    ("vue", "<template>\n  <button @click=\"count++\">{{ count }}</button>\n</template>\n\n<script setup>\nimport { ref } from 'vue'\nconst count = ref(0)\n</script>\n"),
    ("svelte", "<script>\n  let count = 0;\n</script>\n\n<button on:click={() => count++}>\n  {count}\n</button>\n"),
    ("diff", "--- a/file.txt\n+++ b/file.txt\n@@ -1,3 +1,3 @@\n line one\n-line two\n+line 2\n"),
    ("ini", "[server]\nhost = localhost\nport = 8080\n\n[database]\nuser = admin\n"),
    ("ini", "; settings\n[general]\nenabled = yes\nname = My App\n"),
    ("javascript", "document.querySelector('#btn').addEventListener('click', () => {\n  alert('hi');\n});\n"),
    ("javascript", "module.exports = {\n  mode: 'production',\n  entry: './src/index.js',\n};\n"),
    ("javascript", "async function load() {\n  const res = await fetch('/api/items');\n  if (!res.ok) throw new Error(res.statusText);\n  return res.json();\n}\n"),
    ("typescript", "type Result<T> = { ok: true; value: T } | { ok: false; error: string };\n"),
    ("typescript", "export class Cache<K, V> {\n  private store = new Map<K, V>();\n  get(key: K): V | undefined {\n    return this.store.get(key);\n  }\n}\n"),
    ("tsx", "const Item = ({ name }: Props) => (\n  <li className=\"item\">{name}</li>\n);\n"),
    ("jsx", "function Hello(props) {\n  return <h1>Hello, {props.name}</h1>;\n}\n"),
    ("python", "class Stack:\n    def __init__(self):\n        self.items = []\n\n    def push(self, item):\n        self.items.append(item)\n"),
    ("python", "with open('data.txt') as f:\n    lines = [line.strip() for line in f if line]\nprint(len(lines))\n"),
    ("python", "@app.route('/')\ndef index():\n    return render_template('index.html')\n"),
    ("java", "List<String> names = new ArrayList<>();\nnames.add(\"Ada\");\nfor (String n : names) {\n    System.out.println(n);\n}\n"),
    ("java", "@RestController\npublic class HelloController {\n    @GetMapping(\"/\")\n    public String hello() {\n        return \"hi\";\n    }\n}\n"),
    ("c", "int sum(int *xs, size_t n) {\n    int total = 0;\n    for (size_t i = 0; i < n; i++)\n        total += xs[i];\n    return total;\n}\n"),
    ("c", "char *buf = malloc(len + 1);\nif (!buf) return NULL;\nmemcpy(buf, src, len);\nbuf[len] = '\\0';\n"),
    ("cpp", "std::vector<int> v{3, 1, 2};\nstd::sort(v.begin(), v.end());\nfor (auto x : v) std::cout << x << '\\n';\n"),
    ("cpp", "class Shape {\npublic:\n    virtual ~Shape() = default;\n    virtual double area() const = 0;\n};\n"),
    ("csharp", "var names = people.Where(p => p.Age > 18).Select(p => p.Name).ToList();\n"),
    ("csharp", "public async Task<IActionResult> Get(int id)\n{\n    var item = await _db.Items.FindAsync(id);\n    return item == null ? NotFound() : Ok(item);\n}\n"),
    ("go", "type User struct {\n\tName string `json:\"name\"`\n\tAge  int    `json:\"age\"`\n}\n"),
    ("go", "for i, v := range items {\n\tfmt.Printf(\"%d: %s\\n\", i, v)\n}\n"),
    ("rust", "#[derive(Debug, Clone)]\nstruct Point {\n    x: f64,\n    y: f64,\n}\n"),
    ("rust", "let names: Vec<String> = users.iter().map(|u| u.name.clone()).collect();\n"),
    ("rust", "match value {\n    Some(x) if x > 0 => println!(\"positive\"),\n    Some(_) => println!(\"other\"),\n    None => {}\n}\n"),
    ("php", "<?php\nforeach ($users as $user) {\n    echo $user['name'] . \"\\n\";\n}\n"),
    ("php", "public function store(Request $request)\n{\n    $post = Post::create($request->all());\n    return redirect()->route('posts.show', $post);\n}\n"),
    ("ruby", "def greet(name = \"world\")\n  \"Hello, #{name}!\"\nend\n"),
    ("ruby", "class User < ApplicationRecord\n  has_many :posts\n  validates :email, presence: true\nend\n"),
    ("kotlin", "val user = repository.findById(id) ?: return null\nprintln(user.name)\n"),
    ("kotlin", "class MainActivity : AppCompatActivity() {\n    override fun onCreate(savedInstanceState: Bundle?) {\n        super.onCreate(savedInstanceState)\n    }\n}\n"),
    ("swift", "let numbers = [1, 2, 3]\nlet doubled = numbers.map { $0 * 2 }\nprint(doubled)\n"),
    ("swift", "class ViewController: UIViewController {\n    override func viewDidLoad() {\n        super.viewDidLoad()\n    }\n}\n"),
    ("bash", "#!/bin/bash\nset -e\nnpm install\nnpm run build\n"),
    ("bash", "while read -r line; do\n  echo \"${line^^}\"\ndone < input.txt\n"),
    ("bash", "export PATH=\"$HOME/.local/bin:$PATH\"\nalias ll='ls -la'\n"),
    ("powershell", "function Get-Greeting {\n    param([string]$Name = 'World')\n    \"Hello, $Name\"\n}\n"),
    ("powershell", "Install-Module -Name PSReadLine -Force\nSet-ExecutionPolicy RemoteSigned -Scope CurrentUser\n"),
    ("sql", "UPDATE accounts SET balance = balance - 100 WHERE id = 42;\n"),
    ("sql", "select u.name, count(o.id) as orders\nfrom users u\nleft join orders o on o.user_id = u.id\ngroup by u.name;\n"),
    ("html", "<!DOCTYPE html>\n<html>\n<head><title>Test</title></head>\n<body></body>\n</html>\n"),
    ("html", "<form action=\"/login\" method=\"post\">\n  <input type=\"text\" name=\"user\">\n  <button type=\"submit\">Go</button>\n</form>\n"),
    ("css", "@media (max-width: 600px) {\n  .sidebar {\n    display: none;\n  }\n}\n"),
    ("css", "h1 {\n  font-size: 2rem;\n  color: #333;\n  margin-bottom: 1em;\n}\n"),
    ("scss", "@mixin flex-center {\n  display: flex;\n  align-items: center;\n}\n.box { @include flex-center; }\n"),
    ("scss", ".card {\n  padding: 1rem;\n  .title {\n    font-weight: bold;\n  }\n  &:hover { opacity: 0.8; }\n}\n"),
    ("json", "{\"error\": {\"code\": 404, \"message\": \"Not found\"}}\n"),
    ("json", "{\n  \"compilerOptions\": {\n    \"target\": \"es2020\",\n    \"strict\": true\n  }\n}\n"),
    ("yaml", "- name: Install deps\n  run: npm ci\n- name: Test\n  run: npm test\n"),
    ("yaml", "apiVersion: v1\nkind: Service\nmetadata:\n  name: web\nspec:\n  ports:\n    - port: 80\n"),
    ("toml", "[tool.poetry]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[tool.poetry.dependencies]\npython = \"^3.11\"\n"),
    ("toml", "[[bin]]\nname = \"cli\"\npath = \"src/main.rs\"\n"),
    ("markdown", "Some intro text.\n\n## Features\n\n* Fast\n* Small\n\nSee the [docs](./docs.md).\n"),
    ("markdown", "# Changelog\n\n## 1.0.0\n\n- Initial release\n"),
    ("dockerfile", "FROM python:3.12-slim\nCOPY requirements.txt .\nRUN pip install -r requirements.txt\nEXPOSE 8000\n"),
    ("dockerfile", "FROM golang:1.22 AS build\nWORKDIR /src\nRUN go build -o /app\n\nFROM scratch\nCOPY --from=build /app /app\nENTRYPOINT [\"/app\"]\n"),
    ("makefile", "test:\n\tcargo test --all\n\nlint:\n\tcargo clippy -- -D warnings\n\n.PHONY: test lint\n"),
    ("makefile", "SRC := $(wildcard *.c)\nOBJ := $(SRC:.c=.o)\n\napp: $(OBJ)\n\t$(CC) -o $@ $^\n"),
    ("lua", "function M.setup(opts)\n  opts = opts or {}\n  vim.keymap.set('n', '<leader>f', M.find)\nend\n"),
    ("lua", "local count = 0\nwhile count < 10 do\n  count = count + 1\nend\nprint(count)\n"),
    ("r", "df <- data.frame(x = 1:5, y = c(2, 4, 6, 8, 10))\nmodel <- lm(y ~ x, data = df)\nsummary(model)\n"),
    ("r", "library(dplyr)\nmtcars %>%\n  filter(cyl == 4) %>%\n  summarise(mpg = mean(mpg))\n"),
    ("dart", "Future<void> fetchData() async {\n  final response = await http.get(Uri.parse(url));\n  print(response.body);\n}\n"),
    ("dart", "@override\nWidget build(BuildContext context) {\n  return Scaffold(body: Center(child: Text('Hi')));\n}\n"),
    ("scala", "val evens = (1 to 10).filter(_ % 2 == 0)\nprintln(evens.mkString(\", \"))\n"),
    ("scala", "def greet(name: String): String = s\"Hello, $name\"\n\nobject App {\n  def main(args: Array[String]): Unit = println(greet(\"x\"))\n}\n"),
    ("haskell", "fact :: Integer -> Integer\nfact 0 = 1\nfact n = n * fact (n - 1)\n"),
    ("haskell", "data Tree a = Leaf | Node (Tree a) a (Tree a)\n  deriving (Show, Eq)\n"),
    ("elixir", "def handle_call(:get, _from, state) do\n  {:reply, state, state}\nend\n"),
    ("elixir", "case File.read(path) do\n  {:ok, body} -> body\n  {:error, reason} -> raise reason\nend\n"),
    ("perl", "#!/usr/bin/perl\nuse strict;\nprint \"Hello\\n\";\n"),
    ("perl", "open(my $fh, '<', $file) or die \"Cannot open $file: $!\";\nwhile (my $line = <$fh>) {\n    chomp $line;\n}\n"),
    ("xml", "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<note><to>Tove</to></note>\n"),
    ("xml", "<configuration>\n  <appSettings>\n    <add key=\"Mode\" value=\"Debug\" />\n  </appSettings>\n</configuration>\n"),
    ("vue", "<template>\n  <div>{{ message }}</div>\n</template>\n\n<script>\nexport default {\n  data() { return { message: 'Hi' } }\n}\n</script>\n"),
    ("svelte", "<script>\n  export let items = [];\n</script>\n\n{#each items as item}\n  <p>{item}</p>\n{/each}\n"),
    ("diff", "diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-Hello\n+Hello, world\n"),
    ("ini", "[Settings]\nTheme=Dark\nFontSize=12\n"),
    ("ini", "[mysqld]\ndatadir = /var/lib/mysql\nsocket = /var/run/mysqld/mysqld.sock\n"),
    ("javascript", "const app = express();\napp.get('/', (req, res) => res.send('ok'));\napp.listen(3000);\n"),
    ("javascript", "import { readFile } from 'node:fs/promises';\nconst text = await readFile('a.txt', 'utf8');\n"),
    ("typescript", "function sum(xs: number[]): number {\n  return xs.reduce((a, b) => a + b, 0);\n}\n"),
    ("typescript", "enum Direction {\n  Up,\n  Down,\n}\nconst d: Direction = Direction.Up;\n"),
    ("tsx", "export default function Page({ params }: { params: { id: string } }) {\n  return <main>{params.id}</main>;\n}\n"),
    ("jsx", "const List = ({ items }) => (\n  <ul>\n    {items.map((i) => <li key={i}>{i}</li>)}\n  </ul>\n);\n"),
    ("python", "import numpy as np\n\nx = np.linspace(0, 1, 100)\ny = np.sin(x)\n"),
    ("python", "try:\n    value = int(text)\nexcept ValueError:\n    value = 0\n"),
    ("python", "names = {k: v for k, v in pairs if v is not None}\n"),
    ("java", "public interface Repository<T> {\n    Optional<T> findById(long id);\n    void save(T item);\n}\n"),
    ("java", "try (BufferedReader r = new BufferedReader(new FileReader(path))) {\n    String line = r.readLine();\n} catch (IOException e) {\n    e.printStackTrace();\n}\n"),
    ("c", "static void swap(int *a, int *b) {\n    int t = *a;\n    *a = *b;\n    *b = t;\n}\n"),
    ("c", "#define MAX(a, b) ((a) > (b) ? (a) : (b))\ntypedef unsigned char u8;\n"),
    ("cpp", "auto ptr = std::make_shared<Widget>(42);\nptr->draw();\n"),
    ("cpp", "namespace app {\nclass Logger {\n  public:\n    void log(const std::string& msg);\n};\n}\n"),
    ("csharp", "namespace Demo;\n\npublic record Point(int X, int Y);\n"),
    ("csharp", "foreach (var item in items)\n{\n    Console.WriteLine($\"{item.Name}: {item.Price:C}\");\n}\n"),
    ("go", "ctx, cancel := context.WithTimeout(context.Background(), time.Second)\ndefer cancel()\n"),
    ("go", "func add(a, b int) int {\n\treturn a + b\n}\n"),
    ("rust", "use std::collections::HashMap;\n\nlet mut counts = HashMap::new();\n*counts.entry(word).or_insert(0) += 1;\n"),
    ("rust", "pub trait Shape {\n    fn area(&self) -> f64;\n}\n"),
    ("php", "<?php\n\nnamespace App\\Models;\n\nclass User extends Model\n{\n    protected $fillable = ['name'];\n}\n"),
    ("php", "$items = array_map(fn($x) => $x * 2, $items);\nprint_r($items);\n"),
    ("ruby", "require 'json'\ndata = JSON.parse(File.read('a.json'))\nputs data['name']\n"),
    ("ruby", "users.select { |u| u.active? }.map(&:email)\n"),
    ("kotlin", "suspend fun fetch(): String = withContext(Dispatchers.IO) {\n    api.load()\n}\n"),
    ("kotlin", "when (x) {\n    1 -> println(\"one\")\n    else -> println(\"other\")\n}\n"),
    ("swift", "struct User: Codable {\n    let id: Int\n    var name: String?\n}\n"),
    ("swift", "if let url = URL(string: path) {\n    print(url.host ?? \"\")\n}\n"),
    ("bash", "#!/usr/bin/env bash\nfor i in {1..5}; do echo $i; done\n"),
    ("bash", "case \"$1\" in\n  start) run ;;\n  stop) halt ;;\n  *) echo usage ;;\nesac\n"),
    ("powershell", "$services = Get-Service | Where-Object { $_.Status -eq 'Running' }\n$services | Format-Table Name\n"),
    ("powershell", "try {\n    Invoke-WebRequest -Uri $url -OutFile $out\n} catch {\n    Write-Error $_\n}\n"),
    ("sql", "INSERT INTO users (name, email) VALUES ('Ada', 'ada@example.com');\n"),
    ("sql", "SELECT COUNT(*) FROM orders WHERE created_at >= NOW() - INTERVAL '7 days';\n"),
    ("html", "<p>Hello <strong>world</strong>, see <a href=\"https://example.com\">this</a>.</p>\n"),
    ("html", "<table>\n  <tr><th>Name</th><th>Age</th></tr>\n  <tr><td>Ada</td><td>36</td></tr>\n</table>\n"),
    ("css", ".btn:hover {\n  background-color: #0056b3;\n  transform: translateY(-1px);\n}\n"),
    ("css", ":root {\n  --primary: #3b82f6;\n  --radius: 4px;\n}\n"),
    ("scss", "$breakpoint: 768px;\n\n@media (min-width: $breakpoint) {\n  .grid { display: grid; }\n}\n"),
    ("scss", "// Buttons\n.btn {\n  @extend %base;\n  color: white;\n}\n"),
    ("json", "[\n  1,\n  2,\n  3\n]\n"),
    ("json", "{\n  \"scripts\": {\n    \"build\": \"tsc\",\n    \"test\": \"jest\"\n  },\n  \"devDependencies\": {}\n}\n"),
    ("yaml", "server:\n  port: 8080\n  host: 0.0.0.0\nlogging:\n  level: info\n"),
    ("yaml", "---\nlanguage: node_js\nnode_js:\n  - \"18\"\n  - \"20\"\n"),
    ("toml", "name = \"app\"\ndebug = false\nworkers = 4\n"),
    ("toml", "[server]\nhost = \"0.0.0.0\"\nport = 8080\n\n[database]\nurl = \"postgres://localhost/db\"\n"),
    ("markdown", "1. First step\n2. Second step\n\n> **Tip:** read the `README` first.\n"),
    ("markdown", "### API\n\n`highlight(code, lang)` returns HTML.\n\n| Name | Type |\n|------|------|\n| code | string |\n"),
    ("dockerfile", "FROM nginx:alpine\nCOPY dist/ /usr/share/nginx/html/\n"),
    ("dockerfile", "FROM ubuntu:22.04\nRUN apt-get update && apt-get install -y curl\nENV LANG=C.UTF-8\nCMD [\"bash\"]\n"),
    ("makefile", "install:\n\tnpm install\n\nclean:\n\trm -rf dist\n"),
    ("makefile", "PREFIX ?= /usr/local\n\ninstall: app\n\tinstall -m 755 app $(PREFIX)/bin\n"),
    ("lua", "local M = {}\n\nfunction M.add(a, b)\n  return a + b\nend\n\nreturn M\n"),
    ("lua", "for i, v in ipairs(list) do\n  print(i, v)\nend\n"),
    ("r", "x <- c(1, 2, 3)\nmean(x)\nplot(x, type = \"l\")\n"),
    ("r", "for (i in 1:10) {\n  if (i %% 2 == 0) print(i)\n}\n"),
    ("dart", "String greet(String name) => 'Hello, $name';\n\nvoid main() => print(greet('Dart'));\n"),
    ("dart", "final List<int> numbers = [1, 2, 3];\nnumbers.forEach((n) => print(n));\n"),
    ("scala", "trait Animal {\n  def speak(): String\n}\nclass Dog extends Animal {\n  def speak() = \"Woof\"\n}\n"),
    ("scala", "val result = list match {\n  case Nil => 0\n  case head :: tail => head\n}\n"),
    ("haskell", "import qualified Data.Map as Map\n\nmain :: IO ()\nmain = putStrLn \"hi\"\n"),
    ("haskell", "quicksort [] = []\nquicksort (p:xs) = quicksort [x | x <- xs, x < p] ++ [p] ++ quicksort [x | x <- xs, x >= p]\n"),
    ("elixir", "defmodule Math do\n  def sum(a, b), do: a + b\nend\n"),
    ("elixir", "Enum.each(list, fn item ->\n  IO.inspect(item)\nend)\n"),
    ("perl", "my $str = \"hello world\";\n$str =~ s/world/perl/;\nprint \"$str\\n\";\n"),
    ("perl", "sub greet {\n    my ($name) = @_;\n    return \"Hi $name\";\n}\n"),
    ("xml", "<?xml version=\"1.0\"?>\n<rss version=\"2.0\"><channel><title>Feed</title></channel></rss>\n"),
    ("xml", "<dependency>\n  <groupId>junit</groupId>\n  <artifactId>junit</artifactId>\n  <version>4.13</version>\n</dependency>\n"),
    ("vue", "<template>\n  <p v-if=\"show\">Visible</p>\n</template>\n\n<style scoped>\np { color: red; }\n</style>\n"),
    ("svelte", "<script>\n  let name = 'world';\n</script>\n\n<input bind:value={name}>\n<p>Hello {name}!</p>\n"),
    ("diff", "@@ -10,7 +10,7 @@ fn main() {\n-    let x = 1;\n+    let x = 2;\n     println!(\"{x}\");\n"),
    ("diff", "--- old.txt\t2024-01-01\n+++ new.txt\t2024-01-02\n@@ -1,2 +1,2 @@\n-a\n+b\n c\n"),
    ("ini", "[Unit]\nDescription=thing\n\n[extras]\nkey=value\n"),
    ("ini", "[database]\nhost=localhost\nport=5432\nname=app\n"),
    ("javascript", "const sleep = (ms) => new Promise((r) => setTimeout(r, ms));\nawait sleep(100);\n"),
    ("javascript", "$(document).ready(function () {\n  $('.menu').toggleClass('open');\n});\n"),
    ("typescript", "const config: Record<string, number> = { retries: 3 };\nexport default config;\n"),
    ("typescript", "async function getUser(id: string): Promise<User | null> {\n  return db.users.find(id);\n}\n"),
    ("tsx", "export const Card = ({ title, children }: CardProps) => {\n  return <section className=\"card\"><h2>{title}</h2>{children}</section>;\n};\n"),
    ("jsx", "ReactDOM.render(<App />, document.getElementById('root'));\n"),
    ("python", "if __name__ == \"__main__\":\n    main()\n"),
    ("python", "from dataclasses import dataclass\n\n@dataclass\nclass Point:\n    x: float\n    y: float\n"),
    ("python", "result = [x ** 2 for x in range(10) if x % 2 == 0]\nprint(result)\n"),
    ("java", "Map<String, Integer> counts = new HashMap<>();\ncounts.put(\"a\", 1);\n"),
    ("java", "public static int factorial(int n) {\n    return n <= 1 ? 1 : n * factorial(n - 1);\n}\n"),
    ("c", "int main(int argc, char **argv) {\n    if (argc < 2) {\n        fprintf(stderr, \"usage\\n\");\n        return 1;\n    }\n}\n"),
    ("c", "struct node {\n    int value;\n    struct node *next;\n};\n"),
    ("cpp", "for (const auto& [key, value] : map) {\n    std::cout << key << \": \" << value << '\\n';\n}\n"),
    ("cpp", "#include <vector>\n#include <algorithm>\n\nint main() {\n    std::vector<int> v;\n}\n"),
    ("csharp", "private readonly ILogger<HomeController> _logger;\n\npublic HomeController(ILogger<HomeController> logger)\n{\n    _logger = logger;\n}\n"),
    ("csharp", "string path = Path.Combine(dir, \"a.txt\");\nFile.WriteAllText(path, \"hi\");\n"),
    ("go", "resp, err := http.Get(url)\nif err != nil {\n\tlog.Fatal(err)\n}\ndefer resp.Body.Close()\n"),
    ("go", "ch := make(chan int)\ngo func() { ch <- 1 }()\nfmt.Println(<-ch)\n"),
    ("rust", "fn parse(s: &str) -> Result<u32, ParseIntError> {\n    s.trim().parse::<u32>()\n}\n"),
    ("rust", "let v: Vec<i32> = (1..=10).filter(|x| x % 2 == 0).collect();\n"),
    ("php", "<?php echo htmlspecialchars($title); ?>\n"),
    ("php", "$pdo = new PDO($dsn, $user, $pass);\n$stmt = $pdo->prepare('SELECT * FROM users WHERE id = ?');\n$stmt->execute([$id]);\n"),
    ("ruby", "module Greeting\n  def self.hello(name)\n    \"Hello #{name}\"\n  end\nend\n"),
    ("ruby", "File.open('log.txt', 'a') { |f| f.puts 'done' }\n"),
    ("kotlin", "val list = mutableListOf<Int>()\nfor (i in 1..5) list.add(i)\nprintln(list.sum())\n"),
    ("kotlin", "fun String.shout() = uppercase() + \"!\"\n\nfun main() = println(\"hi\".shout())\n"),
    ("swift", "enum Direction {\n    case north, south\n}\nvar d = Direction.north\nd = .south\n"),
    ("swift", "func fetch(completion: @escaping (Data?) -> Void) {\n    URLSession.shared.dataTask(with: url) { data, _, _ in\n        completion(data)\n    }.resume()\n}\n"),
    ("bash", "if [[ -f ~/.bashrc ]]; then\n  source ~/.bashrc\nfi\n"),
    ("bash", "git log --oneline | head -n 5\ncurl -sSL https://example.com/install.sh | sh\n"),
    ("powershell", "$env:PATH += \";C:\\tools\"\nWrite-Output \"Updated\"\n"),
    ("powershell", "Get-Process | Sort-Object CPU -Descending | Select-Object -First 5\n"),
    ("sql", "DROP TABLE IF EXISTS sessions;\nCREATE INDEX idx_users_email ON users (email);\n"),
    ("sql", "WITH recent AS (\n  SELECT * FROM events WHERE ts > now() - interval '1 day'\n)\nSELECT type, count(*) FROM recent GROUP BY type;\n"),
    ("html", "<nav>\n  <a href=\"/\">Home</a> |\n  <a href=\"/blog\">Blog</a>\n</nav>\n"),
    ("html", "<img src=\"cat.png\" alt=\"A cat\">\n<br>\n<span class=\"caption\">Cat</span>\n"),
    ("css", "* {\n  box-sizing: border-box;\n}\nbody {\n  margin: 0;\n}\n"),
    ("css", ".grid {\n  display: grid;\n  grid-template-columns: repeat(3, 1fr);\n  gap: 1rem;\n}\n"),
    ("scss", "$sizes: (small: 12px, large: 24px);\n@each $name, $size in $sizes {\n  .text-#{$name} { font-size: $size; }\n}\n"),
    ("json", "{\"name\":\"x\",\"items\":[1,2,3],\"nested\":{\"ok\":true}}\n"),
    ("json", "[\n  {\n    \"id\": 1,\n    \"title\": \"Buy milk\",\n    \"done\": false\n  }\n]\n"),
    ("yaml", "# Default values\nreplicaCount: 1\nimage:\n  repository: nginx\n  tag: \"1.25\"\n"),
    ("yaml", "dependencies:\n  flutter:\n    sdk: flutter\n  http: ^1.1.0\n"),
    ("toml", "[build]\ntarget = \"wasm32-unknown-unknown\"\n\n[profile.release]\nlto = true\nopt-level = \"s\"\n"),
    ("markdown", "**Note**: this project is in *beta*.\n\n## Install\n\n    npm i colorfulcode\n"),
    ("markdown", "- [ ] Write docs\n- [x] Ship it\n\nSee [issue #12](https://github.com/x/y/issues/12).\n"),
    ("dockerfile", "FROM rust:1.80 as builder\nWORKDIR /usr/src/app\nCOPY . .\nRUN cargo install --path .\n"),
    ("makefile", ".PHONY: run\nrun:\n\tpython app.py\n"),
    ("lua", "local Player = {}\nPlayer.__index = Player\n\nfunction Player.new(name)\n  return setmetatable({ name = name }, Player)\nend\n"),
    ("r", "ggplot(df, aes(x = year, y = value)) +\n  geom_line() +\n  theme_minimal()\n"),
    ("r", "result <- tryCatch({\n  risky()\n}, error = function(e) NA)\n"),
    ("dart", "class Person {\n  final String name;\n  Person(this.name);\n}\n"),
    ("dart", "import 'package:flutter/material.dart';\n\nvoid main() => runApp(const MyApp());\n"),
    ("scala", "import scala.concurrent.Future\n\nval f: Future[Int] = Future { 42 }\n"),
    ("scala", "sealed trait Shape\ncase class Circle(r: Double) extends Shape\n"),
    ("haskell", "main = do\n  name <- getLine\n  putStrLn (\"Hi \" ++ name)\n"),
    ("haskell", "instance Show Color where\n  show Red = \"red\"\n  show Blue = \"blue\"\n"),
    ("elixir", "defp validate(%{email: email} = params) when is_binary(email) do\n  {:ok, params}\nend\n"),
    ("elixir", "children = [\n  {Phoenix.PubSub, name: MyApp.PubSub},\n  MyAppWeb.Endpoint\n]\n"),
    ("perl", "my @sorted = sort { $a <=> $b } @numbers;\nprint join(', ', @sorted), \"\\n\";\n"),
    ("perl", "use LWP::Simple;\nmy $content = get($url);\ndie \"Couldn't get it!\" unless defined $content;\n"),
    ("xml", "<svg:rect xmlns:svg=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"/>\n<items>\n  <item id=\"1\"/>\n</items>\n"),
    ("xml", "<settings>\n  <server>\n    <id>central</id>\n    <username>me</username>\n  </server>\n</settings>\n"),
    ("vue", "<script setup>\nconst props = defineProps(['msg'])\n</script>\n\n<template>\n  <h1>{{ props.msg }}</h1>\n</template>\n"),
    ("svelte", "<script>\n  import { onMount } from 'svelte';\n  let data;\n  onMount(async () => { data = await load(); });\n</script>\n\n{#if data}\n  <p>{data.title}</p>\n{/if}\n"),
    ("diff", "diff --git a/main.go b/main.go\nindex 1234567..89abcde 100644\n--- a/main.go\n+++ b/main.go\n@@ -3,4 +3,4 @@\n-\tfmt.Println(\"a\")\n+\tfmt.Println(\"b\")\n"),
    ("gitconfig", "[core]\n\trepositoryformatversion = 0\n\tfilemode = true\n\tbare = false\n"),
    ("ini", "; php.ini\n[PHP]\nmemory_limit = 128M\nmax_execution_time = 30\n"),
];

/// A few lines are often ambiguous (`x = f(y)` is valid in a dozen
/// languages), so a handful of misses is allowed.
#[test]
fn short_snippets() {
    let wrong: Vec<String> = SNIPPETS
        .iter()
        .filter(|(id, code)| !is_correct(id, detected(code)))
        .map(|(id, code)| format!("{id} detected as {}: {code:?}", detected(code)))
        .collect();
    let right = SNIPPETS.len() - wrong.len();
    println!("{right} of {} snippets detected", SNIPPETS.len());
    assert!(right * 100 >= SNIPPETS.len() * 97, "{} of {} snippets wrong: {wrong:#?}", wrong.len(), SNIPPETS.len());
}

#[test]
fn not_confident() {
    let prose = [
        "",
        "   \n\n",
        "hello",
        "just some words",
        "Let me know where you listen to music.",
        "Just some plain text.",
        "The quick brown fox jumps over the lazy dog.\n",
        "Meeting notes: we agreed to ship on Friday and review the numbers next week.",
        "Hello,\n\nThanks for the update. I will take a look tomorrow and get back to you.\n\nBest,\nSam\n",
        "TODO\n- buy milk\n- call mom\n",
        "Release 1.2 fixes a crash when opening large files and improves startup time.",
        "Q: Why is the sky blue?\nA: Because of Rayleigh scattering.\n",
    ];
    for text in prose {
        assert_eq!(Language::detect(text), None, "{text:?}");
    }
}

#[test]
fn among_candidates() {
    let find = |id| Language::find(id).unwrap();
    let code = "let x = 1;\nconsole.log(x);\n";
    assert_eq!(Language::detect_among(code, &[find("javascript"), find("python")]), Some(find("javascript")));
    assert_eq!(Language::detect_among(code, &[find("python"), find("ruby")]), None);
    assert_eq!(Language::detect_among(code, &[]), None);
    // Shebangs still win, as long as they name a candidate.
    let script = "#!/bin/sh\necho hi\n";
    assert_eq!(Language::detect_among(script, &[find("bash")]), Some(find("bash")));
    assert_eq!(Language::detect_among(script, &[find("python")]), None);
}

#[test]
fn unknown_names_do_not_guess() {
    assert!(Language::find("auto").is_none());
    assert!(Language::find("not a language").is_none());
}
