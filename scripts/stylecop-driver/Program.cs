// Usage: StyleCopDriver <folder>. Parses every .cs file below the folder into one compilation (no references but the core
// library) and runs every StyleCop analyzer over it with every rule on as a warning; prints one JSON object per finding:
// {"file", "rule", "line", "message"}, the file relative to the folder.
using System.Collections.Immutable;
using System.Reflection;
using System.Text.Json;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.Diagnostics;

var root = Path.GetFullPath(args[0]);
var files = Directory.EnumerateFiles(root, "*.cs", SearchOption.AllDirectories).Order(StringComparer.Ordinal).ToList();
var parse = new CSharpParseOptions(LanguageVersion.Latest);
var trees = files.AsParallel().AsOrdered().Select(f => CSharpSyntaxTree.ParseText(File.ReadAllText(f), parse, f)).ToList();

var assembly = Assembly.LoadFrom(Path.Combine(AppContext.BaseDirectory, "StyleCop.Analyzers.dll"));
var analyzers = assembly.GetTypes()
    .Where(t => !t.IsAbstract && typeof(DiagnosticAnalyzer).IsAssignableFrom(t) && t.GetCustomAttributes<DiagnosticAnalyzerAttribute>().Any())
    .Select(t => (DiagnosticAnalyzer)Activator.CreateInstance(t)!)
    .ToImmutableArray();
var on = analyzers.SelectMany(a => a.SupportedDiagnostics).Select(d => d.Id).Distinct().ToDictionary(i => i, _ => ReportDiagnostic.Warn);
var options = new CSharpCompilationOptions(OutputKind.DynamicallyLinkedLibrary).WithSpecificDiagnosticOptions(on);
var compilation = CSharpCompilation.Create("corpus", trees, [MetadataReference.CreateFromFile(typeof(object).Assembly.Location)], options);
var driver = compilation.WithAnalyzers(analyzers, new CompilationWithAnalyzersOptions(
    new AnalyzerOptions(ImmutableArray<AdditionalText>.Empty), null, concurrentAnalysis: true, logAnalyzerExecutionTime: false));
var found = driver.GetAnalyzerDiagnosticsAsync().GetAwaiter().GetResult();
var output = Console.Out;
foreach (var d in found.OrderBy(d => d.Location.SourceTree?.FilePath, StringComparer.Ordinal).ThenBy(d => d.Location.SourceSpan.Start))
{
    if (!d.Location.IsInSource || !(d.Id.StartsWith("SA") || d.Id.StartsWith("SX"))) continue;
    var span = d.Location.GetLineSpan();
    output.WriteLine(JsonSerializer.Serialize(new
    {
        file = Path.GetRelativePath(root, d.Location.SourceTree!.FilePath).Replace('\\', '/'),
        rule = d.Id,
        line = span.StartLinePosition.Line + 1,
        message = d.GetMessage(),
    }));
}
