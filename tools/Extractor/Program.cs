// Extractor de solo lectura: copia a extract/ los archivos de ER cuyo path empieza por
// alguno de los prefijos dados. Uso:
//   Extractor <carpetaGame> <salida> list            -> resumen de carpetas
//   Extractor <carpetaGame> <salida> <prefijo>...   -> extrae
//   Extractor ui <raíz del proyecto> [salida]        -> imágenes de interfaz de extract/menu/hi (ya
//                                                       extraído) a extract/ui: iconos de objetos y
//                                                       bendiciones de data/, pantallas de carga, atlas
//                                                       de menús con sus recortes y teselas del mapa
using System.Text.Json;
using System.Text.RegularExpressions;
using Andre.IO.VFS;
using Andre.Core;
using SoulsFormats;

// SoulsFormats busca oo2core (copiado junto a la herramienta) en el directorio actual.
var rutas = args.Take(3).Select(Path.GetFullPath).ToArray();
Directory.SetCurrentDirectory(AppContext.BaseDirectory);

// Teselas del mapa con todas sus variantes (una por combinación de fragmentos) y las máscaras
// (71_maptile.mtmskbnd), para montar el mapa descubierto sin costuras (tools/map_background.py).
if (args[0] == "maptile")
{
    var raiz = rutas[1];
    var menu = Path.Combine(raiz, "extract", "menu");
    var dst = Path.Combine(raiz, "extract", "maptiles_todas");
    Directory.CreateDirectory(dst);
    // La máscara de cada tesela (id = nivel·10000 + x·100 + y) es la unión de los fragmentos que la
    // tocan: la variante con esa máscara es la tesela con todo descubierto. Las demás combinaciones
    // (y bits sueltos que no están en la máscara) no casan con sus vecinas.
    var completas = new HashSet<string>();
    foreach (var f in BND4.Read(Path.Combine(menu, "71_maptile.mtmskbnd.dcx")).Files)
    {
        var txt = System.Text.Encoding.UTF8.GetString(f.Bytes.ToArray());
        File.WriteAllText(Path.Combine(dst, Path.GetFileName(f.Name)), txt);
        var mapa = Path.GetFileNameWithoutExtension(f.Name);
        foreach (Match m in Regex.Matches(txt, @"id=""(\d+)"" mask=""(\d+)"""))
        {
            int id = int.Parse(m.Groups[1].Value);
            completas.Add($"{mapa}_L{id / 10000}_{id / 100 % 100:D2}_{id % 100:D2}_{uint.Parse(m.Groups[2].Value):x8}");
        }
    }
    int nt = 0;
    foreach (var f in BXF4.Read(Path.Combine(menu, "71_maptile.tpfbhd"), Path.Combine(menu, "71_maptile.tpfbdt")).Files)
        foreach (var t in TPF.Read(f.Bytes).Textures.Where(t => t.Name.StartsWith("MENU_MapTile_M00_") || t.Name.StartsWith("MENU_MapTile_M10_")))
        {
            // Sin entrada en la máscara, la tesela no tiene fragmentos: vale la _00000000.
            var clave = Regex.Replace(t.Name, "_[0-9a-f]{8}$", "");
            bool conMascara = completas.Any(c => c.StartsWith(clave + "_"));
            if (completas.Contains(t.Name) || (!conMascara && t.Name.EndsWith("_00000000")))
            {
                File.WriteAllBytes(Path.Combine(dst, t.Name + ".dds"), t.Bytes);
                nt++;
            }
        }
    Console.WriteLine($"{nt} teselas descubiertas en {dst}");
    return;
}

if (args[0] == "ui")
{
    var raiz = rutas[1];
    var ui = args.Length > 2 ? rutas[2] : Path.Combine(raiz, "extract", "ui");
    var hi = Path.Combine(raiz, "extract", "menu", "hi");
    // Todo "icono": N de los datos del mod.
    var iconos = new[] { "data/objetos/pool.json", "data/reliquias.json" }
        .SelectMany(f => Regex.Matches(File.ReadAllText(Path.Combine(raiz, f)), @"""icono"":\s*(\d+)").Select(m => int.Parse(m.Groups[1].Value)))
        .Where(i => i > 0)
        .Select(i => $"MENU_Knowledge_{i:D5}")
        .ToHashSet();
    foreach (var d in new[] { "iconos_dds", "carga_dds", "sb" }) Directory.CreateDirectory(Path.Combine(ui, d));
    var solo = BXF4.Read(Path.Combine(hi, "00_solo.tpfbhd"), Path.Combine(hi, "00_solo.tpfbdt"));
    var hechos = new HashSet<string>();
    int cargas = 0;
    foreach (var f in solo.Files)
    {
        var nombre = Path.GetFileName(f.Name).Replace(".tpf.dcx", "");
        if (iconos.Contains(nombre))
        {
            foreach (var t in TPF.Read(f.Bytes).Textures) File.WriteAllBytes(Path.Combine(ui, "iconos_dds", nombre + ".dds"), t.Bytes);
            hechos.Add(nombre);
        }
        else if (nombre.StartsWith("MENU_Load_"))
        {
            foreach (var t in TPF.Read(f.Bytes).Textures) { File.WriteAllBytes(Path.Combine(ui, "carga_dds", t.Name + ".dds"), t.Bytes); cargas++; }
        }
    }
    // Atlas de los menús: cada SB_* con su .layout (dónde está cada pieza).
    int atlas = 0;
    foreach (var t in TPF.Read(Path.Combine(hi, "01_common.tpf.dcx")).Textures) { File.WriteAllBytes(Path.Combine(ui, "sb", t.Name + ".dds"), t.Bytes); atlas++; }
    foreach (var f in BND4.Read(Path.Combine(hi, "01_common.sblytbnd.dcx")).Files) File.WriteAllBytes(Path.Combine(ui, "sb", Path.GetFileName(f.Name)), f.Bytes.ToArray());
    // Teselas del mapa del mundo (fondo del mapa de la run, tools/map_background.py).
    var teselas = Path.Combine(Path.GetDirectoryName(ui)!, "maptiles");
    Directory.CreateDirectory(teselas);
    var menu = Path.GetDirectoryName(hi)!;
    int nt = 0;
    foreach (var f in BXF4.Read(Path.Combine(menu, "71_maptile.tpfbhd"), Path.Combine(menu, "71_maptile.tpfbdt")).Files)
        foreach (var t in TPF.Read(f.Bytes).Textures.Where(t => t.Name.StartsWith("MENU_MapTile_M00_") && t.Name.EndsWith("_00000000")))
        {
            File.WriteAllBytes(Path.Combine(teselas, t.Name + ".dds"), t.Bytes);
            nt++;
        }
    Console.WriteLine($"{hechos.Count} de {iconos.Count} iconos, {cargas} pantallas de carga, {atlas} atlas y {nt} teselas del mapa en {ui}");
    foreach (var falta in iconos.Except(hechos).Order()) Console.WriteLine($"no está en 00_solo: {falta}");
    return;
}

var game = rutas[0];
var outDir = rutas[1];
// Nightreign si la carpeta contiene nightreign.exe (misma lógica que ER con su diccionario y claves):
// de ahí salen los jefes que porta ModBuilder (data/nightreign.json, docs/JEFES_Y_MOVESETS_DE_OTROS_JUEGOS.md).
var nr = File.Exists(Path.Combine(game, "nightreign.exe"));
var gameId = nr ? Game.NR : Game.ER;
var dictText = File.ReadAllText(Path.Combine(AppContext.BaseDirectory, nr ? "EldenRingNightreignDictionary.txt" : "EldenRingDictionary.txt"));
var dict = new Andre.Core.Util.BhdDictionary(dictText, BHD5.Game.EldenRing);
// «@dlc01» entre los prefijos: solo ese archivo (el DLC de Nightreign trae su propio allmaterial con el
// mismo nombre que el de la base, y si no gana el de la base).
var soloArchivos = args.Skip(2).Where(x => x.StartsWith('@')).Select(x => x[1..]).ToArray();
var binders = Andre.Formats.BinderArchive.FindBHDs(game, gameId)
    .Where(s => soloArchivos.Length == 0 || soloArchivos.Any(x => Path.GetFileNameWithoutExtension(s).Equals(x, StringComparison.OrdinalIgnoreCase)))
    .Select(s => new Andre.Formats.BinderArchive(s, s.Replace(".bhd5", ".bdt").Replace(".bhd", ".bdt"), gameId))
    .ToArray();
using var vfs = new ArchiveBinderVirtualFileSystem(binders, dict);
var all = vfs.FileHeaders.Select(h => h.Item1).Distinct().ToList();

if (args[2] == "list")
{
    foreach (var g in all.GroupBy(p => string.Join('/', p.Trim('/').Split('/').Take(p.Count(c => c == '/') > 2 ? 2 : 1)))
                         .OrderBy(g => g.Key))
        Console.WriteLine($"{g.Count(),6}  {g.Key}");
    return;
}

var prefixes = args.Skip(2).Where(x => !x.StartsWith('@')).Select(p => p.ToLower().Trim('/')).ToArray();
long bytes = 0; int n = 0;
foreach (var p in all.Where(p => prefixes.Any(pre => p.TrimStart('/').StartsWith(pre + "/") || p.TrimStart('/').StartsWith(pre))))
{
    var data = vfs.ReadFileOrThrow(p);
    var dest = Path.Combine(outDir, p.TrimStart('/').Replace('/', Path.DirectorySeparatorChar));
    Directory.CreateDirectory(Path.GetDirectoryName(dest)!);
    File.WriteAllBytes(dest, data.ToArray());
    bytes += data.Length; n++;
}
Console.WriteLine($"{n} archivos, {bytes / 1048576.0:F1} MB");
