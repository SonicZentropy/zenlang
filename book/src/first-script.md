# Your First Script

Create a file called `hello.zen`:

```rust
print("Hello, Zenlang!");
```

Run it:

```bash
zenc run hello.zen
```

You should see `Hello, Zenlang!` printed to the terminal.

## REPL

Start an interactive session:

```bash
zenc repl
```

Try some expressions:

```
> let x = 42;
> print(x * 2);
84
> fn factorial(n) { if n <= 1 { 1 } else { n * factorial(n - 1) } }
> factorial(10)
3628800
```

## Creating a New Project

```bash
zenc new my_project
cd my_project
zenc run
```

This creates:

```
my_project/
  src/
    main.zen     # Entry point
  zenc.json      # Project configuration
```
