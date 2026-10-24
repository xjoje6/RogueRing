// Construye la carpeta del mod (paquete de me3) a partir de los archivos vanilla de extract/:
// eventos de viaje a las arenas (la DLL solo enciende un flag), animaciones y comportamiento del
// jugador, efectos de las bendiciones en regulation.bin y el punto de aparición del coliseo.
//
// Uso: ModBuilder <raíz del proyecto> (con oo2core_6_win64.dll del juego copiado junto al .exe)
using System.Text.Json;
using Andre.Formats;
using SoulsFormats;

// Siempre con lista explícita: con params, C# confunde el primer argumento con layerMask.
static EMEVD.Instruction Ins(int bank, int id, params object[] a) => new EMEVD.Instruction(bank, id, (IEnumerable<object>)a);

static System.Numerics.Vector3 V3(JsonElement a) => new(a[0].GetSingle(), a[1].GetSingle(), a[2].GetSingle());

// data/reacciones.json: lo que el evento del mapa original de un jefe le hace cuando él mismo se pone un
// efecto (el injerto de Godrick, el Nihil de Mohg...). Un evento por reacción y hueco (45000990 + h) con
// las instrucciones copiadas del original, la entidad del jefe cambiada por la del hueco y la condición
// «el del hueco tiene el efecto». Se inicializan en `iniciar` (el constructor del mapa).
static int Reacciones(string root, EMEVD destino, Action<EMEVD.Instruction> iniciar)
{
    var d = JsonDocument.Parse(File.ReadAllText(Path.Combine(root, "data", "reacciones.json")));
    var origen = new Dictionary<string, EMEVD>();
    int r = 0;
    foreach (var x in d.RootElement.GetProperty("reacciones").EnumerateArray())
    {
        var mapa = x.GetProperty("mapa").GetString()!;
        if (!origen.TryGetValue(mapa, out var em)) origen[mapa] = em = EMEVD.Read(Path.Combine(root, "extract", "event", mapa + ".emevd.dcx"));
        var ev = em.Events.First(e => e.ID == x.GetProperty("evento").GetInt64());
        var ent = x.GetProperty("entidad").GetUInt32();
        var si = x.GetProperty("si").GetInt32();
        var hasta = Math.Min(x.GetProperty("hasta").GetInt32(), ev.Instructions.Count - 1);
        var sin = x.GetProperty("sin").EnumerateArray().Select(v => v.GetInt32()).ToHashSet();
        for (uint h = 0; h < 10; h++)
        {
            var hueco = 45_000_990 + h;
            var nuevo = new EMEVD.Event(79_620_000 + r * 10 + h, EMEVD.Event.RestBehaviorType.Restart);
            // IF Character Has SpEffect (principal, hueco, efecto, sí, igual, 1)
            nuevo.Instructions.Add(Ins(4, 5, new object[] { (sbyte)0, hueco, si, (byte)1, (byte)0, 1f }));
            for (int k = x.GetProperty("desde").GetInt32(); k <= hasta; k++)
            {
                if (sin.Contains(k)) continue;
                var arg = ev.Instructions[k].ArgData.ToArray();
                for (int o = 0; o + 4 <= arg.Length; o += 4)
                    if (BitConverter.ToUInt32(arg, o) == ent) BitConverter.GetBytes(hueco).CopyTo(arg, o);
                nuevo.Instructions.Add(new EMEVD.Instruction(ev.Instructions[k].Bank, ev.Instructions[k].ID, arg));
            }
            destino.Events.Add(nuevo);
            iniciar(Ins(2000, 0, new object[] { 0, (uint)nuevo.ID, 0u }));
        }
        r++;
    }
    return r;
}

/// Giro (grados, eje Y del MSB) para mirar en la dirección d: con 0 se mira a -Z y con 180 a +Z (el
/// punto de aparición del coliseo).
static float Mirar(System.Numerics.Vector3 d) => (float)(Math.Atan2(-d.X, -d.Z) * 180 / Math.PI);


var root = Path.GetFullPath(args[0]);
// Plantilla de enemigo de las variantes del coliseo (Plantilla(), más abajo).
MSBE? plantillaMsb = null;
// CharaInitID de los PNJ humanos (CharaInit, más abajo).
Dictionary<int, int>? charaInit = null;
// SoulsFormats busca oo2core (copiado junto a la herramienta) en el directorio actual.
Directory.SetCurrentDirectory(AppContext.BaseDirectory);

// Prueba suelta: variante del coliseo con un jefe cualquiera colocado (ver VarianteColiseo).
// Uso: ModBuilder <raíz> coliseo_con <chr> <npc> <think> <entidad> <salida.msb.dcx>
if (args.Length > 1 && args[1] == "coliseo_con")
{
    VarianteColiseo(new() { (int.Parse(args[2]), int.Parse(args[3]), int.Parse(args[4]), 0) }, args[6]);
    Console.WriteLine($"escrito {args[6]}");
    return;
}

// Un coliseo por combate (docs/COOP_SALAS.md): copia del MSB del coliseo del mod con los enemigos del
// encuentro COLOCADOS, como enemigos del mapa. Los enemigos del mapa los sincroniza el juego en
// cooperativo; los de spawn_chr, no. La DLL copia la variante sobre el coliseo antes de cada viaje.
// Hueco i: entidad 45000990 + i. Oleada w: grupo de entidades 45000900 + w, que el evento del mapa
// tiene apagado hasta que la DLL enciende el flag 1049300110 + w (docs/LISTAS_Y_COMBATES.md).
// PNJ humanos (modelo c0000: Vyke, Ymir, el Oso rojo del DLC...): su aspecto y su equipo vienen del
// CharaInitID de su colocación en el mapa, no del modelo. Sin él salían invisibles (23-10). Se saca
// de su colocación original en los mapas del juego y se guarda en data/chara_init.json (NPCParamID →
// CharaInitID), para no leer los 1347 mapas en cada construcción.
int CharaInit(int npc)
{
    var ruta = Path.Combine(root, "data", "chara_init.json");
    if (charaInit == null && File.Exists(ruta))
        charaInit = JsonSerializer.Deserialize<Dictionary<string, int>>(File.ReadAllText(ruta))!.ToDictionary(k => int.Parse(k.Key), k => k.Value);
    if (charaInit != null && charaInit.TryGetValue(npc, out var ci)) return ci;
    charaInit ??= new();
    foreach (var f in Directory.GetFiles(Path.Combine(root, "extract", "map", "mapstudio"), "*.msb.dcx"))
    {
        MSBE m;
        try { m = MSBE.Read(f); } catch { continue; }
        foreach (var e in m.Parts.Enemies.Where(e => e.ModelName == "c0000" && e.CharaInitID > 0 && e.NPCParamID > 0))
            charaInit.TryAdd(e.NPCParamID, e.CharaInitID);
    }
    File.WriteAllText(ruta, JsonSerializer.Serialize(charaInit.OrderBy(k => k.Key).ToDictionary(k => k.Key.ToString(), k => k.Value), new JsonSerializerOptions { WriteIndented = true }));
    if (!charaInit.ContainsKey(npc)) Console.WriteLine($"AVISO: PNJ {npc} sin CharaInitID en los mapas del juego");
    return charaInit.GetValueOrDefault(npc, -1);
}

// La plantilla se lee una vez (leer Stormveil en cada variante hacía tardar 11 min con las parejas del
// cooperativo, 24-10); no se modifica: se copian sus piezas.
MSBE Plantilla() => plantillaMsb ??= MSBE.Read(Path.Combine(root, "extract", "map", "mapstudio", "m10_00_00_00.msb.dcx"));

void VarianteColiseo(List<(int chr, int npc, int think, int ola)> enemigos, string salida)
{
    var col = MSBE.Read(Path.Combine(root, "mod", "map", "mapstudio", "m45_00_00_00.msb.dcx"));
    // Plantilla: un enemigo corriente de Stormveil (sus campos raros valen para cualquier mapa).
    var tpl = Plantilla();
    var plantilla = tpl.Parts.Enemies.First(e => e.ModelName != "c0000" && e.ChrActivateCondParamID == 0 && e.EntityID == 0);
    var suelo = col.Parts.Collisions.OrderBy(c => c.Position.Length()).First();
    for (int i = 0; i < enemigos.Count; i++)
    {
        var (chr, npc, think, ola) = enemigos[i];
        var modelo = $"c{chr:D4}";
        var en = (MSBE.Part.Enemy)plantilla.DeepCopy();
        en.Name = $"{modelo}_{9000 + i}";
        en.ModelName = modelo;
        en.NPCParamID = npc;
        if (chr == 0) en.CharaInitID = CharaInit(npc);
        en.ThinkParamID = think;
        en.EntityID = (uint)(45000990 + i);
        en.EntityGroupIDs = new uint[8];
        en.EntityGroupIDs[0] = (uint)(45000900 + ola);
        en.PlatoonID = 0;
        en.WalkRouteName = "";
        en.CollisionPartName = suelo.Name;
        // Se dibuja con los grupos de la sala en la que está (copiados del suelo).
        en.DisplayDataStruct = suelo.DisplayDataStruct.DeepCopy();
        en.GparamConfigStruct = suelo.GparamConfigStruct.DeepCopy();
        en.TileLoadConfig = suelo.TileLoadConfig.DeepCopy();
        // Donde lo creaba spawn_chr: 15 m delante del punto de aparición (0, 0, -10), mirándolo. Los de
        // una misma oleada, en filas de cuatro separados 3,5 m.
        int k = enemigos.Take(i).Count(x => x.ola == ola), enOla = enemigos.Count(x => x.ola == ola);
        int fila = k / 4, cols = Math.Min(4, enOla - fila * 4);
        float x = (k % 4 - (cols - 1) / 2f) * 3.5f;
        en.Position = new System.Numerics.Vector3(x, 0, 5 + fila * 3);
        en.Rotation = new System.Numerics.Vector3(0, 0, 0);
        if (!col.Models.Enemies.Any(m => m.Name == modelo))
        {
            var m = (MSBE.Model.Enemy)tpl.Models.Enemies.First(x => x.Name == plantilla.ModelName).DeepCopy();
            m.SibPath = m.SibPath.Replace(plantilla.ModelName, modelo);
            m.Name = modelo;
            col.Models.Enemies.Add(m);
        }
        col.Parts.Enemies.Add(en);
    }
    Directory.CreateDirectory(Path.GetDirectoryName(Path.GetFullPath(salida))!);
    col.Write(salida, col.Compression);
}

// Arenas grandes (data/arenas_grandes.json, 18-10): como el coliseo, pero en el mapa de una arena de
// jefe grande. La variante es el mapa sin sus enemigos, con los del encuentro alrededor del centro (en
// filas de cuatro, mirando al jugador) y el punto de aparición del viaje. Cada enemigo se dibuja y pisa
// como el jefe original más cercano al centro (sus grupos de dibujado y su colisión).
void VarianteArena(ArenaGrande a, List<(int chr, int npc, int think, int ola)> enemigos, string salida)
{
    var msb = MSBE.Read(Path.Combine(root, "extract", "map", "mapstudio", a.Nombre + ".msb.dcx"));
    var tpl = Plantilla();
    var plantilla = tpl.Parts.Enemies.First(e => e.ModelName != "c0000" && e.ChrActivateCondParamID == 0 && e.EntityID == 0);
    var sala = msb.Parts.Enemies.FirstOrDefault(e => a.Sala != 0 && e.EntityID == a.Sala)
        ?? msb.Parts.Enemies.OrderBy(e => System.Numerics.Vector3.Distance(e.Position, a.Centro)).First();
    var quitados = msb.Parts.Enemies.Select(e => e.Name).ToHashSet();
    msb.Parts.Enemies.Clear();
    QuitarReferencias(msb, quitados);
    // La entidad del viaje no puede existir ya en el mapa: con 13000990 (un punto de Farum Azula junto a
    // la gracia de Maliketh) el viaje iba allí y no a la arena (18-10).
    var choca = msb.Parts.GetEntries().Select(p => (p.Name, (long)p.EntityID)).Concat(msb.Regions.GetEntries().Select(x => (x.Name, (long)x.EntityID)))
        .FirstOrDefault(x => x.Item2 == a.Spawn);
    if (choca.Name != null) throw new Exception($"arena {a.Clave}: la entidad de aparición {a.Spawn} ya es {choca.Name} en {a.Nombre}");
    // Punto de aparición: copia del jugador del mapa más cercano (su sala), con la entidad del viaje.
    var pj = (MSBE.Part.Player)msb.Parts.Players.OrderBy(p => System.Numerics.Vector3.Distance(p.Position, a.Aparicion)).First().DeepCopy();
    pj.Name = "c0000_9990";
    pj.EntityID = a.Spawn;
    pj.Position = a.Aparicion;
    var haciaCentro = a.Centro - a.Aparicion;
    pj.Rotation = new System.Numerics.Vector3(0, Mirar(haciaCentro), 0);
    msb.Parts.Players.Add(pj);
    // Ejes: f hacia el lado contrario al jugador (las filas se alejan), r de lado.
    var f = System.Numerics.Vector3.Normalize(new System.Numerics.Vector3(haciaCentro.X, 0, haciaCentro.Z));
    var r = new System.Numerics.Vector3(f.Z, 0, -f.X);
    for (int i = 0; i < enemigos.Count; i++)
    {
        var (chr, npc, think, ola) = enemigos[i];
        var modelo = $"c{chr:D4}";
        var en = (MSBE.Part.Enemy)plantilla.DeepCopy();
        en.Name = $"{modelo}_{9000 + i}";
        en.ModelName = modelo;
        en.NPCParamID = npc;
        if (chr == 0) en.CharaInitID = CharaInit(npc);
        en.ThinkParamID = think;
        en.EntityID = (uint)(45000990 + i);
        en.EntityGroupIDs = new uint[8];
        en.EntityGroupIDs[0] = (uint)(45000900 + ola);
        en.PlatoonID = 0;
        en.WalkRouteName = "";
        en.CollisionPartName = sala.CollisionPartName;
        en.DisplayDataStruct = sala.DisplayDataStruct.DeepCopy();
        en.GparamConfigStruct = sala.GparamConfigStruct.DeepCopy();
        en.TileLoadConfig = sala.TileLoadConfig.DeepCopy();
        int k = enemigos.Take(i).Count(x => x.ola == ola), enOla = enemigos.Count(x => x.ola == ola);
        int fila = k / 4, cols = Math.Min(4, enOla - fila * 4);
        float x = (k % 4 - (cols - 1) / 2f) * 4f;
        en.Position = a.Centro + r * x + f * (fila * 4f);
        en.Rotation = new System.Numerics.Vector3(0, Mirar(-haciaCentro), 0);
        if (!msb.Models.Enemies.Any(m => m.Name == modelo))
        {
            var m = (MSBE.Model.Enemy)tpl.Models.Enemies.First(x => x.Name == plantilla.ModelName).DeepCopy();
            m.SibPath = m.SibPath.Replace(plantilla.ModelName, modelo);
            m.Name = modelo;
            msb.Models.Enemies.Add(m);
        }
        msb.Parts.Enemies.Add(en);
    }
    Directory.CreateDirectory(Path.GetDirectoryName(Path.GetFullPath(salida))!);
    msb.Write(salida, msb.Compression);
}

/// Lo que nombra a partes quitadas del MSB (los enemigos de una arena grande): los eventos del mapa que
/// las usan (escuadras, monturas, generadores...) fuera; en regiones y partes, la referencia vacía. Si
/// no, SoulsFormats no escribe el MSB («references X, which does not exist»).
static void QuitarReferencias(MSBE msb, HashSet<string> quitados)
{
    bool Nombra(object o) => o.GetType().GetProperties()
        .Where(pr => pr.CanRead && pr.GetIndexParameters().Length == 0)
        .Any(pr => pr.PropertyType == typeof(string) && pr.Name != "Name" && quitados.Contains(pr.GetValue(o) as string ?? "")
                || pr.PropertyType == typeof(string[]) && (pr.GetValue(o) as string[] ?? []).Any(quitados.Contains));
    void Vaciar(object o)
    {
        foreach (var pr in o.GetType().GetProperties().Where(pr => pr.CanRead && pr.CanWrite && pr.GetIndexParameters().Length == 0 && pr.Name != "Name"))
        {
            if (pr.PropertyType == typeof(string) && quitados.Contains(pr.GetValue(o) as string ?? "")) pr.SetValue(o, null);
            if (pr.PropertyType == typeof(string[]) && pr.GetValue(o) is string[] v && v.Any(quitados.Contains))
                pr.SetValue(o, v.Select(x => quitados.Contains(x) ? null : x).ToArray());
        }
    }
    foreach (var pr in msb.Events.GetType().GetProperties())
        if (pr.GetValue(msb.Events) is System.Collections.IList lista)
            for (int i = lista.Count - 1; i >= 0; i--)
                if (lista[i] is { } ev && Nombra(ev)) lista.RemoveAt(i);
    foreach (var r in msb.Regions.GetEntries()) Vaciar(r);
    foreach (var pa in msb.Parts.GetEntries()) Vaciar(pa);
}

/// Enemigos de un encuentro de data/acts con su oleada: `enemigos` + `oleadas`, o el de chr/npc/think.
List<(int chr, int npc, int think, int ola)> EnemigosDe(JsonElement e)
{
    var lista = new List<(int, int, int, int)>();
    if (e.TryGetProperty("enemigos", out var ens) && ens.GetArrayLength() > 0)
    {
        var ola = new Dictionary<int, int>();
        if (e.TryGetProperty("oleadas", out var os))
        {
            int w = 0;
            foreach (var o in os.EnumerateArray())
            {
                foreach (var i in o.EnumerateArray()) ola[i.GetInt32()] = w;
                if (o.GetArrayLength() > 0) w++;
            }
        }
        int idx = 0;
        foreach (var x in ens.EnumerateArray())
        {
            lista.Add((x.GetProperty("chr").GetInt32(), x.GetProperty("npc").GetInt32(), x.GetProperty("think").GetInt32(), ola.GetValueOrDefault(idx, 0)));
            idx++;
        }
        // Dúos con «el último en pie»: la segunda fase de cada uno, apagada en su oleada (3 + k) y en
        // los huecos que siguen (Encounter::huecos_fase2 en la DLL).
        if (e.TryGetProperty("ultimo_en_pie", out var ult))
        {
            int k = 0;
            foreach (var u in ult.EnumerateArray())
                if (u.TryGetProperty("fase2", out var f2) && f2.ValueKind == JsonValueKind.Object)
                    lista.Add((f2.GetProperty("chr").GetInt32(), f2.GetProperty("npc").GetInt32(), f2.GetProperty("think").GetInt32(), 3 + k++));
        }
    }
    else if (e.TryGetProperty("chr", out var chr))
    {
        int npc = e.TryGetProperty("npc", out var n) && n.ValueKind == JsonValueKind.Number ? n.GetInt32() : -1;
        int think = e.TryGetProperty("think", out var t) && t.ValueKind == JsonValueKind.Number ? t.GetInt32() : -1;
        lista.Add((chr.GetInt32(), npc, think, 0));
    }
    return lista;
}

var src = Path.Combine(root, "extract", "event", "common.emevd.dcx");
var dst = Path.Combine(root, "mod", "event", "common.emevd.dcx");

var emevd = EMEVD.Read(src);
var init = emevd.Events.First(e => e.ID == 0);


long nextId = 79_600_000; // rango propio de eventos de RogueRing
var arenas = Directory.GetFiles(Path.Combine(root, "data"), "arenas_*.json").OrderBy(f => f)
    .SelectMany(f => JsonDocument.Parse(File.ReadAllText(f)).RootElement.GetProperty("arenas").EnumerateArray().ToList());
foreach (var a in arenas)
{
    uint flag = a.GetProperty("flag").GetUInt32();
    var m = a.GetProperty("map").EnumerateArray().Select(x => (byte)x.GetInt32()).ToArray();
    uint spawn = a.GetProperty("spawn").GetUInt32();

    var ev = new EMEVD.Event(nextId, EMEVD.Event.RestBehaviorType.Default);
    // IF Event Flag (grupo 0 = principal, ON, tipo flag, id)
    ev.Instructions.Add(Ins(3, 0, (sbyte)0, (byte)1, (byte)0, flag));
    // Set Event Flag (tipo flag, id, OFF)
    ev.Instructions.Add(Ins(2003, 66, (byte)0, flag, (byte)0));
    // Warp Player (área, bloque, región, índice, entidad de aparición, sin cartel de zona)
    ev.Instructions.Add(Ins(2003, 14, m[0], m[1], m[2], m[3], spawn, 0));
    // END Unconditionally (Restart): vuelve a escuchar el flag
    ev.Instructions.Add(Ins(1000, 4, (byte)1));
    emevd.Events.Add(ev);

    // Initialize Event (ranura 0, id, parámetro 0) en el constructor de common
    init.Instructions.Add(Ins(2000, 0, 0, (uint)nextId, 0u));
    nextId++;
}

// Música de combate: por cada tema de data/musica.json, tres eventos que esperan su flag (empezar,
// subir de intensidad y parar) y llaman a Set Boss BGM. La DLL elige el tema y enciende el flag.
{
    var temas = JsonDocument.Parse(File.ReadAllText(Path.Combine(root, "data", "musica.json"))).RootElement
        .GetProperty("temas").EnumerateArray().Select(x => x.GetInt32()).ToArray();
    if (temas.Length > 100) throw new Exception("musica.json: más de 100 temas (los flags van de 100 en 100)");
    long idEv = 79_680_000;
    for (int k = 0; k < temas.Length; k++)
        foreach (var (baseFlag, estado) in new[] { (1_049_300_500u, 0), (1_049_300_600u, 1), (1_049_300_700u, -2) })
        {
            uint flag = baseFlag + (uint)k;
            var ev = new EMEVD.Event(idEv, EMEVD.Event.RestBehaviorType.Default);
            ev.Instructions.Add(Ins(3, 0, (sbyte)0, (byte)1, (byte)0, flag));
            ev.Instructions.Add(Ins(2003, 66, (byte)0, flag, (byte)0));
            ev.Instructions.Add(Ins(2010, 10, temas[k], estado));
            ev.Instructions.Add(Ins(1000, 4, (byte)1));
            emevd.Events.Add(ev);
            init.Instructions.Add(Ins(2000, 0, 0, (uint)idEv, 0u));
            idEv++;
        }
    Console.WriteLine($"música: {temas.Length} temas");
}

Directory.CreateDirectory(Path.GetDirectoryName(dst)!);
emevd.Write(dst, emevd.Compression);
Console.WriteLine($"escrito {dst} ({emevd.Compression})");

// Comprobación: releer y contar.
var check = EMEVD.Read(dst);
Console.WriteLine($"releído: {check.Events.Count} eventos (vanilla {EMEVD.Read(src).Events.Count})");

// --- Recompensas: una fila de ItemLot reservada que la DLL reescribe en memoria antes de entregar ---
const int REWARD_LOT = 79_600_000;
const uint REWARD_FLAG = 1049300190;
{
    var ev = new EMEVD.Event(79_690_000, EMEVD.Event.RestBehaviorType.Default);
    ev.Instructions.Add(Ins(3, 0, (sbyte)0, (byte)1, (byte)0, REWARD_FLAG));
    ev.Instructions.Add(Ins(2003, 66, (byte)0, REWARD_FLAG, (byte)0));
    ev.Instructions.Add(Ins(2003, 4, REWARD_LOT));            // Award Item Lot
    ev.Instructions.Add(Ins(1000, 4, (byte)1));               // END Restart
    emevd.Events.Add(ev);
    init.Instructions.Add(Ins(2000, 0, 0, (uint)ev.ID, 0u));

    // Sonidos de menú del juego para las pantallas del mod: flag 1049300160+k → PlaySE(10000, f (2), MENU_SE[k]).
    // Los ids salen de cs_main.bnk (eventos play_fXXXXXXXXX); el orden lo usa la DLL (overlay::Se).
    int[] menuSe = { 50100, 200, 400, 100, 500000, 320, 0, 310, 1000, 3000, 30000, 40900, 10000, 40000 };
    for (int k = 0; k < menuSe.Length; k++)
    {
        uint flag = 1_049_300_160u + (uint)k;
        var es = new EMEVD.Event(79_690_200 + k, EMEVD.Event.RestBehaviorType.Default);
        es.Instructions.Add(Ins(3, 0, (sbyte)0, (byte)1, (byte)0, flag));
        es.Instructions.Add(Ins(2003, 66, (byte)0, flag, (byte)0));
        es.Instructions.Add(Ins(2010, 2, 10000u, 2, menuSe[k]));
        es.Instructions.Add(Ins(1000, 4, (byte)1));
        emevd.Events.Add(es);
        init.Instructions.Add(Ins(2000, 0, 0, (uint)es.ID, 0u));
    }

    emevd.Write(dst, emevd.Compression);
}

// Animaciones de otros juegos: colección de Blubber (nexusmods.com/eldenring/mods/3027, uso libre con
// crédito), ya adaptadas al esqueleto del jugador. Van en recursos/ (fuera de git); si faltan, el mod
// se queda con las del juego.
// - Monje: la voltereta ligera (27100-27103: delante, detrás, izquierda, derecha) pasa a ser la
//   esquiva de la Duquesa de Nightreign (27160-27163 de su c0000, extract_nr): su animación y los
//   eventos de su TAE, que dejan volver a esquivar a los 0,27 s y atacar a los 0,23 s (la voltereta de
//   ER, a los 0,70 s): así se encadena. De sus SpEffect solo quedan los que usa también la voltereta de
//   ER (los de la Duquesa no existen aquí). Solo él rueda ligero (tools/hks/rogue.hks).
// - Samurái: el desvío perfecto usa los desvíos de Lobo (Sekiro). En el juego, sus animaciones toman
//   prestado el .hkx del golpe a la guardia normal; aquí pasan a tener el suyo, así que la guardia
//   normal no cambia. Solo él desvía (102000). Golpes pequeños y medios: ocho huecos (tipo de guardia
//   0-3 del golpe pequeño, 19800-19830, y del medio, 19801-19831) que tools/hks/rogue.hks elige según
//   el lado del golpe. Sus desvíos de Lobo, según dónde llega la mano:
//   paran por la izquierda 120101, 120104, 122111; por la derecha 120102, 120105, 122112 (empiezan
//   donde acaban los de la izquierda: son la respuesta encadenada); arriba al centro 120100, 120103.
//   Todos llevan los eventos de 19800 para que se jueguen igual. Golpe grande: 19802.
// ESQUIVE_MONJE («duquesa», o la primera de cuatro del paso de Bloodborne a000, p. ej. 7100) y
// DESVIO_SAMURAI (ocho de los huecos en ese orden y el grande, Sekiro a050) cambian la serie.
{
    var recursos = Path.Combine(root, "recursos");
    var esquiveTxt = Environment.GetEnvironmentVariable("ESQUIVE_MONJE") ?? "duquesa";
    var duquesa = esquiveTxt == "duquesa";
    var nrA00 = Path.Combine(root, "extract_nr", "chr", "c0000_a00_hi.anibnd.dcx");
    var nrTae = Path.Combine(root, "extract_nr", "chr", "c0000.anibnd.dcx");
    var esquive = duquesa ? 0 : int.Parse(esquiveTxt);
    // Duquesa (ER ← NR), en huecos que solo usa el monje (tools/hks/rogue.hks; nadie lleva peso):
    // - su esquiva (Lady_Step_TypeA, 27160-27163) en la voltereta ligera;
    // - la encadenada (Lady_Step_TypeB_Selftrans: esquivar durante otra esquiva, la voltereta) en la
    //   voltereta pesada de ER (27120-27127; rogue.hks la pide con el peso pesado si el monje esquiva
    //   antes de 0,7 s de la anterior). La encadenada ligera de ER no sirve: repite 27100-27103
    //   (medido en el juego). DUQUESA_ENCADENADA cambia el grupo;
    // - sus dos pasos atrás agachándose (LadyBackStep 27060 y LadyBackStep2 27070) en el ligero y el
    //   pesado de ER (27000 y 27020), que rogue.hks alterna.
    var encadenada = int.Parse(Environment.GetEnvironmentVariable("DUQUESA_ENCADENADA") ?? "27170");
    var deNr = new Dictionary<int, int> { [27000] = 27060, [27020] = 27070 };
    for (int d = 0; d < 4; d++)
    {
        deNr[27100 + d] = 27160 + d;
        deNr[27120 + d] = encadenada + d;
        deNr[27124 + d] = encadenada + d;
    }
    var paso = (duquesa ? deNr.Keys : Enumerable.Range(27100, 4)).ToDictionary(id => id,
        id => duquesa ? nrA00 : Path.Combine(recursos, "blubber_hunter", "a000 - Base Game Moveset", $"a000_{esquive + id - 27100:D6}.hkx"));
    // Sus .hkx y sus animaciones del TAE (con sus eventos).
    var pasoNr = new Dictionary<int, byte[]>();
    var pasoTae = new Dictionary<int, TAE.Animation>();
    if (duquesa && File.Exists(nrA00) && File.Exists(nrTae))
    {
        var hkx = BND4.Read(nrA00).Files;
        var tae = TAE.Read(BND4.Read(nrTae).Files.First(f => f.Name.EndsWith("a00.tae")).Bytes);
        foreach (var (er, nr) in deNr)
        {
            pasoNr[er] = hkx.First(f => f.Name.EndsWith($"a000_{nr:D6}.hkx")).Bytes.ToArray();
            pasoTae[er] = tae.Animations.First(a => a.ID == nr);
        }
    }
    var desvioIds = (Environment.GetEnvironmentVariable("DESVIO_SAMURAI") ?? "120101,120104,122111,120102,120105,122112,120100,120103,120300")
        .Split(',').Select(int.Parse).ToArray();
    var huecos = new[] { 19800, 19810, 19820, 19830, 19801, 19811, 19821, 19831, 19802 };
    var desvio = Enumerable.Range(0, huecos.Length).ToDictionary(k => huecos[k],
        k => Path.Combine(recursos, "blubber_sekiro", "Kusabimaru", "a050 (Standard Katana Set)", "Deflects", $"a050_{desvioIds[k]:D6}.hkx"));
    var ciclo = huecos[..^1];
    var faltan = paso.Values.Concat(desvio.Values).Count(f => !File.Exists(f)) + (duquesa && pasoNr.Count < deNr.Count ? 1 : 0);
    var chrIn = Path.Combine(root, "extract", "chr");
    var chrOut = Path.Combine(root, "mod", "chr");
    if (faltan > 0)
        Console.WriteLine($"animaciones: faltan {faltan} archivos en recursos/, se quedan las del juego");
    else
    {
        Directory.CreateDirectory(chrOut);
        // Familias de armas con desvío: qué huecos tiene cada una (las que no tienen alguno usan el de a000).
        var porSeccion = new Dictionary<int, List<int>>();
        var taeBnd = BND4.Read(Path.Combine(chrIn, "c0000.anibnd.dcx"));
        foreach (var f in taeBnd.Files.Where(f => f.Name.EndsWith(".tae")))
        {
            if (!int.TryParse(Path.GetFileNameWithoutExtension(f.Name)[1..], out var sec)) continue;
            var tae = TAE.Read(f.Bytes);
            // Esquiva de la Duquesa en la voltereta ligera (a00): sus eventos, sus grupos y sus tiempos;
            // de los SpEffect, solo los que ya pone la voltereta de ER.
            if (sec == 0)
                foreach (var (id, nr) in pasoTae)
                {
                    var er = tae.Animations.First(a => a.ID == id);
                    var deEr = er.Events.Where(e => e.Type is 66 or 67).Select(e => BitConverter.ToInt32(e.GetParameterBytes(false), 0)).ToHashSet();
                    var grupos = nr.EventGroups.ToDictionary(g => g, g => g.GetClone());
                    er.EventGroups = grupos.Values.ToList();
                    er.Events = nr.Events
                        .Where(e => e.Type is not (66 or 67) || deEr.Contains(BitConverter.ToInt32(e.GetParameterBytes(false), 0)))
                        .Select(e =>
                        {
                            var c = e.GetClone(false);
                            if (e.Group != null) c.Group = grupos[e.Group];
                            return c;
                        }).ToList();
                }
            var pequeno = tae.Animations.FirstOrDefault(a => a.ID == 19800);
            if (pequeno == null)
            {
                if (sec == 0 && pasoTae.Count > 0) f.Bytes = tae.Write();
                continue;
            }
            var ids = new List<int>();
            foreach (var a in tae.Animations.Where(a => desvio.ContainsKey((int)a.ID)))
            {
                if (a.MiniHeader is TAE.Animation.AnimMiniHeader.Standard h) h.ImportsHKX = false;
                if (a != pequeno && ciclo.Contains((int)a.ID))
                {
                    var grupos = pequeno.EventGroups.ToDictionary(g => g, g => g.GetClone());
                    a.EventGroups = grupos.Values.ToList();
                    a.Events = pequeno.Events.Select(e =>
                    {
                        var c = e.GetClone(false);
                        if (e.Group != null) c.Group = grupos[e.Group];
                        return c;
                    }).ToList();
                }
                ids.Add((int)a.ID);
            }
            porSeccion[sec] = ids;
            f.Bytes = tae.Write();
        }
        taeBnd.Write(Path.Combine(chrOut, "c0000.anibnd.dcx"));
        // El paso rápido sustituye a la voltereta; los desvíos se añaden a cada familia (id del archivo:
        // 1000000000 + familia * 1000000 + animación, como los del juego).
        void Paquete(string nombre, Func<int, bool> esSuya, bool conPaso)
        {
            var bnd = BND4.Read(Path.Combine(chrIn, $"c0000_{nombre}.anibnd.dcx"));
            var modelo = bnd.Files.First(f => f.Name.EndsWith(".hkx"));
            var dir = Path.GetDirectoryName(modelo.Name)!;
            if (conPaso)
                foreach (var (id, src) in paso)
                    bnd.Files.First(f => f.Name.EndsWith($"a000_{id:D6}.hkx")).Bytes = duquesa ? pasoNr[id] : File.ReadAllBytes(src);
            foreach (var (sec, ids) in porSeccion.Where(p => esSuya(p.Key)))
                foreach (var id in ids)
                {
                    var fid = 1_000_000_000 + sec * 1_000_000 + id;
                    bnd.Files.RemoveAll(f => f.ID == fid);
                    bnd.Files.Add(new BinderFile(modelo.Flags, fid, $@"{dir}\a{sec:D3}_{id:D6}.hkx", File.ReadAllBytes(desvio[id])));
                }
            bnd.Files.Sort((a, b) => a.ID.CompareTo(b.ID));
            bnd.Write(Path.Combine(chrOut, $"c0000_{nombre}.anibnd.dcx"));
        }
        Paquete("a00_hi", s => s == 0, true);
        Paquete("a0x", s => s is > 0 and < 10, false);
        Paquete("a1x", s => s is >= 10 and < 20, false);
        var otras = porSeccion.Keys.Where(s => s >= 20).ToList();
        if (otras.Count > 0) Console.WriteLine($"animaciones: AVISO, familias con desvío sin tratar: {string.Join(",", otras)}");
        Console.WriteLine($"animaciones: esquiva {(duquesa ? $"de la Duquesa (encadenada {encadenada}, pasos atrás 27060/27070)" : $"de Bloodborne {esquive}")} (monje), desvíos {string.Join(",", desvioIds)} (samurái), " +
            $"familias {string.Join(",", porSeccion.Select(p => $"a{p.Key:D2}:{p.Value.Count}"))}");
    }
}

// Caído del cooperativo (22-10; dll/harness/deNr/director/caido.rs y data/caido.json): las
// animaciones de Nightreign de caer, quedarse en el suelo, arrastrarse y levantarse (su estado NearDeath)
// no existen en ER. Van a huecos de animación de evento del jugador (escenas que en el mod no salen): el
// comportamiento ya los conoce y la DLL los pide como ForceAnimationPlayback del EMEVD. Cuál es
// cuál se mira en el juego (pausa de pruebas, «Probar animación»). Sin sus eventos (ver abajo).
{
    var nrA00 = Path.Combine(root, "extract_nr", "chr", "c0000_a00_hi.anibnd.dcx");
    var nrTaeBnd = Path.Combine(root, "extract_nr", "chr", "c0000.anibnd.dcx");
    var huecosCaido = new Dictionary<int, int>
    {
        [60750] = 17440, [60760] = 17460, [60780] = 17470, [60790] = 17480, [60560] = 17302,
        [60800] = 17600, [60810] = 17650, [60811] = 17660, [60530] = 20500,
    };
    var chrIn = Path.Combine(root, "extract", "chr");
    var chrOut = Path.Combine(root, "mod", "chr");
    // Lo que haya dejado ya el bloque de animaciones (esquiva, desvíos) o, si no, lo del juego.
    string Fuente(string n) => File.Exists(Path.Combine(chrOut, n)) ? Path.Combine(chrOut, n) : Path.Combine(chrIn, n);
    if (!File.Exists(nrA00) || !File.Exists(nrTaeBnd))
        Console.WriteLine("caído: sin los archivos de Nightreign, sin sus animaciones");
    else
    {
        Directory.CreateDirectory(chrOut);
        var nrHkx = BND4.Read(nrA00).Files;
        var nrTae = TAE.Read(BND4.Read(nrTaeBnd).Files.First(f => f.Name.EndsWith("a00.tae")).Bytes);
        var taeBnd = BND4.Read(Fuente("c0000.anibnd.dcx"));
        var fTae = taeBnd.Files.First(f => f.Name.EndsWith("a00.tae"));
        var tae = TAE.Read(fTae.Bytes);
        var hechos = new List<string>();
        foreach (var (er, nr) in huecosCaido)
        {
            var deNr = nrTae.Animations.FirstOrDefault(a => a.ID == nr);
            var enEr = tae.Animations.FirstOrDefault(a => a.ID == er);
            if (deNr == null || enEr == null || !nrHkx.Any(f => f.Name.EndsWith($"a000_{nr:D6}.hkx")))
            {
                Console.WriteLine($"caído: AVISO, falta {(deNr == null ? $"la {nr} de NR" : $"el hueco {er}")}");
                continue;
            }
            // Sin eventos: solo el movimiento del cuerpo. Los de NR no son de fiar aquí (22-10, banco: la 17600
            // mató al personaje con el efecto de no morir puesto; es la de caído a fantasma).
            enEr.EventGroups = new();
            enEr.Events = new();
            // Con su propio .hkx (el hueco de ER podía tomar prestado el de otra animación).
            enEr.MiniHeader = deNr.MiniHeader;
            hechos.Add($"{er}←{nr}");
        }
        fTae.Bytes = tae.Write();
        taeBnd.Write(Path.Combine(chrOut, "c0000.anibnd.dcx"));
        foreach (var nombre in new[] { "a00_hi", "a00_md", "a00_lo" })
        {
            var ruta = Fuente($"c0000_{nombre}.anibnd.dcx");
            if (!File.Exists(ruta)) continue;
            var bnd = BND4.Read(ruta);
            var modelo = bnd.Files.First(f => f.Name.EndsWith(".hkx"));
            var dir = Path.GetDirectoryName(modelo.Name)!;
            foreach (var (er, nr) in huecosCaido)
            {
                var hkx = nrHkx.FirstOrDefault(f => f.Name.EndsWith($"a000_{nr:D6}.hkx"));
                if (hkx == null) continue;
                var f = bnd.Files.FirstOrDefault(x => x.Name.EndsWith($"a000_{er:D6}.hkx"));
                if (f != null) f.Bytes = hkx.Bytes.ToArray();
                else if (nombre == "a00_hi") bnd.Files.Add(new BinderFile(modelo.Flags, 1_000_000_000 + er, $@"{dir}\a000_{er:D6}.hkx", hkx.Bytes.ToArray()));
            }
            bnd.Files.Sort((a, b) => a.ID.CompareTo(b.ID));
            bnd.Write(Path.Combine(chrOut, $"c0000_{nombre}.anibnd.dcx"));
        }
        Console.WriteLine($"caído: animaciones de Nightreign {string.Join(", ", hechos)}");
    }
}

// Comportamiento del jugador: el c0000.hks descompilado por la comunidad (github.com/soulsmods/EldenRingHKS,
// clonado en tools/vendor; cuadra con el juego 1.17) con lo nuestro (tools/hks/rogue.hks) al final.
{
    var baseHks = Path.Combine(root, "tools", "vendor", "EldenRingHKS", "c0000.hks");
    var hksOut = Path.Combine(root, "mod", "action", "script", "c0000.hks");
    if (File.Exists(baseHks))
    {
        Directory.CreateDirectory(Path.GetDirectoryName(hksOut)!);
        var texto = File.ReadAllText(baseHks).TrimEnd();
        // Las cinco comprobaciones de la ventana de desvío (102001/11/13/15) pasan por RR_VentanaDesvio,
        // que suma la de la cadena del samurái. Si la base cambia y no aparecen las cinco, se avisa.
        var ventana = new System.Text.RegularExpressions.Regex(
            @"env\(GetSpEffectID, 102001\) == TRUE or env\(GetSpEffectID, 102011\) == TRUE or env\(GetSpEffectID, 102013\)\s*== TRUE or env\(GetSpEffectID, 102015\) == TRUE");
        var n = ventana.Matches(texto).Count;
        if (n != 5) Console.WriteLine($"hks: AVISO, {n} comprobaciones de la ventana de desvío (se esperaban 5)");
        texto = ventana.Replace(texto, "RR_VentanaDesvio() == TRUE");
        texto += "\n" + File.ReadAllText(Path.Combine(root, "tools", "hks", "rogue.hks"));
        File.WriteAllText(hksOut, texto, new System.Text.UTF8Encoding(true));
        Console.WriteLine("hks: c0000 con lo de RogueRing");
    }
    else
        Console.WriteLine("hks: falta tools/vendor/EldenRingHKS (git clone https://github.com/soulsmods/EldenRingHKS)");
}

// Jefes de Nightreign (Nightreign.cs, data/nightreign.json): sus archivos aquí y sus parámetros abajo.
var jefesNr = PortadorNR.Leer(root);
var aparienciasNr = PortadorNR.LeerApariencias(root);
PortadorNR.Archivos(root, jefesNr, aparienciasNr);
PortadorNR.Mensajes(root, aparienciasNr);
// Barras de jefe del juego (dll/harness/fuente/juego.rs, barras_jefe): el nombre de cada combate en los
// textos del juego (NpcName_dlc02, 79700000 en adelante), en español en spaes y spaar (data/i18n/es.json,
// «enemigos») y en inglés en el resto. data/nombres_jefe.json: id del combate → id del texto.
{
    var nombres = new SortedDictionary<string, string>(StringComparer.Ordinal);
    foreach (var f in Directory.GetFiles(Path.Combine(root, "data", "acts"), "*.json").Concat(Directory.GetFiles(Path.Combine(root, "data", "coop"), "*.json")))
        foreach (var e in JsonDocument.Parse(File.ReadAllText(f)).RootElement.GetProperty("encounters").EnumerateArray())
            nombres.TryAdd(e.GetProperty("id").GetString()!, e.GetProperty("name").GetString()!);
    var es = JsonDocument.Parse(File.ReadAllText(Path.Combine(root, "data", "i18n", "es.json"))).RootElement.GetProperty("enemigos");
    var ids = new System.Text.Json.Nodes.JsonObject();
    int k = 0;
    var textos = new List<(int id, string en, string es)>();
    foreach (var (id, nombre) in nombres)
    {
        int fid = 79_700_000 + k++;
        ids[id] = fid;
        textos.Add((fid, nombre, es.TryGetProperty(nombre, out var t) ? t.GetString()! : nombre));
    }
    // Y el de cada enemigo (fila de NpcParam), de su combate de uno o de su manada: en los combates de
    // varios, cada barra lleva el nombre de su enemigo y no el del combate (24-10: en «Espíritu
    // ancestral y Prelado de fuego» las dos barras se llamaban igual). 79750000 en adelante.
    var porNpc = new SortedDictionary<int, string>();
    foreach (var f in Directory.GetFiles(Path.Combine(root, "data", "acts"), "*.json"))
        foreach (var e in JsonDocument.Parse(File.ReadAllText(f)).RootElement.GetProperty("encounters").EnumerateArray())
        {
            var nombre = e.GetProperty("name").GetString()!;
            var grupo = e.TryGetProperty("enemigos", out var en) && en.ValueKind == JsonValueKind.Array && en.GetArrayLength() > 0;
            if (!grupo && e.TryGetProperty("npc", out var np) && np.ValueKind == JsonValueKind.Number)
                porNpc.TryAdd(np.GetInt32(), nombre);
            else if (grupo && en.EnumerateArray().Select(x => x.GetProperty("npc").GetInt32()).Distinct().Count() == 1 && nombre.Contains(" ×"))
                porNpc.TryAdd(en[0].GetProperty("npc").GetInt32(), nombre[..nombre.LastIndexOf(" ×")]);
        }
    // Los que nunca salen solos (los soldados de una horda mezclada): el nombre de su modelo en los alias
    // de Smithbox.
    var alias = JsonDocument.Parse(File.ReadAllText(Path.Combine(root, "tools", "vendor", "Smithbox", "Smithbox.Release", "Output", "Assets", "Aliases", "ER", "Characters.json")))
        .RootElement.EnumerateArray().ToDictionary(a => a.GetProperty("ID").GetString()!, a => a.GetProperty("Name").GetString()!);
    foreach (var f in Directory.GetFiles(Path.Combine(root, "data", "acts"), "*.json").Concat(Directory.GetFiles(Path.Combine(root, "data", "coop"), "*.json")))
        foreach (var e in JsonDocument.Parse(File.ReadAllText(f)).RootElement.GetProperty("encounters").EnumerateArray())
            if (e.TryGetProperty("enemigos", out var en) && en.ValueKind == JsonValueKind.Array)
                foreach (var x in en.EnumerateArray())
                {
                    var npc = x.GetProperty("npc").GetInt32();
                    var chr = x.GetProperty("chr").GetInt32();
                    if (!porNpc.ContainsKey(npc) && chr != 0 && alias.TryGetValue($"c{chr:D4}", out var an)) porNpc[npc] = an;
                }
    var idsNpc = new System.Text.Json.Nodes.JsonObject();
    int j = 0;
    foreach (var (npc, nombre) in porNpc)
    {
        int fid = 79_750_000 + j++;
        idsNpc[npc.ToString()] = fid;
        textos.Add((fid, nombre, es.TryGetProperty(nombre, out var t) ? t.GetString()! : nombre));
    }
    int idiomas = 0;
    var msg = Path.Combine(root, "extract", "msg");
    if (Directory.Exists(msg))
        foreach (var dir in Directory.GetDirectories(msg))
        {
            var lengua = Path.GetFileName(dir);
            var destino = Path.Combine(root, "mod", "msg", lengua, "item_dlc02.msgbnd.dcx");
            var fuente = File.Exists(destino) ? destino : Path.Combine(dir, "item_dlc02.msgbnd.dcx");
            if (!File.Exists(fuente)) continue;
            var bnd = BND4.Read(fuente);
            var f = bnd.Files.FirstOrDefault(x => Path.GetFileNameWithoutExtension(x.Name) == "NpcName_dlc02");
            if (f == null) continue;
            var fmg = FMG.Read(f.Bytes);
            fmg.Entries.RemoveAll(e => e.ID >= 79_700_000 && e.ID < 79_800_000);
            foreach (var (id, en, esp) in textos)
                fmg.Entries.Add(new FMG.Entry(fmg, id, lengua.StartsWith("spa") ? esp : en));
            fmg.Entries.Sort((x, y) => x.ID.CompareTo(y.ID));
            f.Bytes = fmg.Write();
            Directory.CreateDirectory(Path.GetDirectoryName(destino)!);
            bnd.Write(destino);
            idiomas++;
        }
    File.WriteAllText(Path.Combine(root, "data", "nombres_jefe.json"), new System.Text.Json.Nodes.JsonObject
    {
        ["_nota"] = "Lo escribe tools/ModBuilder: id del combate -> id de su nombre en NpcName_dlc02 (barras de jefe del juego). No editar a mano.",
        ["nombres"] = ids,
        ["enemigos"] = idsNpc,
    }.ToJsonString(new JsonSerializerOptions { WriteIndented = true }) + "\n");
    Console.WriteLine($"barras de jefe: {textos.Count} nombres de combate en {idiomas} idiomas");
}

var assets = Path.Combine(root, "tools", "vendor", "Smithbox", "Smithbox.Release", "Output", "Assets", "PARAM", "ER", "Defs");
var reg = SFUtil.DecryptERRegulation(Path.Combine(root, "extract", "regulation.bin"));
Param LoadParam(BinderFile f)
{
    var p = Param.Read(f.Bytes);
    var def = Directory.GetFiles(assets, "*.xml").Select(x => { try { return PARAMDEF.XmlDeserialize(x, true); } catch { return null; } })
        .First(d => d != null && d.ParamType == p.ParamType)!;
    p.ApplyParamdef(def);
    return p;
}
{
    var f = reg.Files.First(x => x.Name.EndsWith("ItemLotParam_map.param"));
    var p = LoadParam(f);
    if (!p.ContainsRow(REWARD_LOT))
    {
        var tpl = p.Rows.First(r => r.ID > 0);
        var row = new Param.Row(tpl, p) { ID = REWARD_LOT, Name = "RogueRing: recompensa (la DLL la reescribe)" };
        for (int k = 1; k <= 8; k++)
        {
            string n = k.ToString("00");
            row[$"lotItemId{n}"]!.Value.SetValue(0);
            row[$"lotItemCategory{n}"]!.Value.SetValue(0);
            row[$"lotItemBasePoint{n}"]!.Value.SetValue((ushort)0);
            row[$"getItemFlagId{n}"]!.Value.SetValue(0u);
        }
        row["lotItemBasePoint01"]!.Value.SetValue((ushort)1000);
        row["lotItemNum01"]!.Value.SetValue((byte)1);
        row["getItemFlagId"]!.Value.SetValue(0u);
        int idx = p.Rows.TakeWhile(r => r.ID < REWARD_LOT).Count();
        p.InsertRow(idx, row);
    }
    f.Bytes = p.Write();
}
// Reglas que cambian el juego base (peso del equipo, escalado de los bosses, viales) NO van aquí: las
// aplica la DLL en memoria al cargar la partida del mod (dll/harness/src/globales.rs).
// Reliquias: un SpEffect permanente por nivel (clon del efecto del Medallón de ámbar carmesí,
// sin icono), con los campos de data/reliquias.json. Id = 79610000 + índice*10 + nivel.
{
    using var rel = JsonDocument.Parse(File.ReadAllText(Path.Combine(root, "data", "reliquias.json")));
    var f3 = reg.Files.First(x => x.Name.EndsWith("SpEffectParam.param"));
    var sp = LoadParam(f3);
    var tpl = sp[310000]!;
    int creados = 0, idx = 0;
    foreach (var r in rel.RootElement.GetProperty("reliquias").EnumerateArray())
    {
        int nivel = 0;
        foreach (var n in r.GetProperty("niveles").EnumerateArray())
        {
            int id = 79_610_000 + idx * 10 + nivel;
            var row = sp[id];
            if (row == null)
            {
                row = new Param.Row(tpl, sp) { ID = id };
                sp.InsertRow(sp.Rows.TakeWhile(x => x.ID < id).Count(), row);
            }
            row.Name = $"RogueRing: {r.GetProperty("id").GetString()} nivel {nivel + 1}";
            row["maxHpRate"]!.Value.SetValue(1.0f);   // la plantilla trae +6 % de vida
            row["iconId"]!.Value.SetValue(-1);
            foreach (var c in n.GetProperty("campos").EnumerateObject())
            {
                var cell = row[c.Name] ?? throw new Exception($"campo desconocido {c.Name}");
                object v = cell.Value switch
                {
                    float => (float)c.Value.GetDouble(),
                    int => c.Value.GetInt32(),
                    byte => (byte)c.Value.GetInt32(),
                    sbyte => (sbyte)c.Value.GetInt32(),
                    short => (short)c.Value.GetInt32(),
                    ushort => (ushort)c.Value.GetInt32(),
                    uint => c.Value.GetUInt32(),
                    _ => throw new Exception($"tipo no soportado en {c.Name}: {cell.Value.GetType()}"),
                };
                cell.SetValue(v);
            }
            creados++;
            nivel++;
        }
        idx++;
    }
    // Desvío perfecto del Samurai: el efecto de la Lágrima dura desviadora (102000), que el HKS consulta,
    // sin duración ni aura (en el mod no hay lágrimas).
    {
        var dv = sp[102000]!;
        dv["effectEndurance"]!.Value.SetValue(-1.0f);
        dv["vfxId"]!.Value.SetValue(-1);
        // La ventana real del desvío (102001/11/13/15, las pone el TAE al cubrirse) multiplica el coste de
        // aguante de la guardia por 0,35 en el juego base: a 0, un desvío acertado no gasta aguante.
        foreach (var id in new[] { 102001, 102011, 102013, 102015 })
            sp[id]!["guardStaminaMult"]!.Value.SetValue(0.0f);
        // Cadena del samurái (7960002, tools/hks/rogue.hks): tras cada desvío acertado, otra ventana de
        // 0,45 s para desviar el golpe siguiente manteniendo la guardia. Como 102011 pero más larga, sin
        // categoría (no debe quitar las marcas de desvío 102019/102020) ni efecto al terminar.
        {
            const int id = 7_960_002;
            var row = sp[id];
            if (row == null)
            {
                row = new Param.Row(sp[102011]!, sp) { ID = id };
                sp.InsertRow(sp.Rows.TakeWhile(x => x.ID < id).Count(), row);
            }
            row.Name = "RogueRing: cadena de desvíos";
            row["effectEndurance"]!.Value.SetValue(0.45f);
            row["replaceSpEffectId"]!.Value.SetValue(-1);
            row["spCategory"]!.Value.SetValue(Convert.ChangeType(0, row["spCategory"]!.Value.Value.GetType()));
            row["categoryPriority"]!.Value.SetValue(Convert.ChangeType(0, row["categoryPriority"]!.Value.Value.GetType()));
            row["guardStaminaMult"]!.Value.SetValue(0.0f);
        }
    }
    // Marcas de personaje para el comportamiento del jugador (tools/hks/rogue.hks): sin efecto. Por
    // debajo de 16777216: el hks trabaja con float de 32 bits y un id mayor llega redondeado.
    foreach (var (id, nombre) in new[] { (7_960_000, "monje"), (7_960_001, "samurai"), (7_960_003, "esquiva del monje") })
    {
        var row = sp[id];
        if (row == null)
        {
            row = new Param.Row(tpl, sp) { ID = id };
            sp.InsertRow(sp.Rows.TakeWhile(x => x.ID < id).Count(), row);
        }
        row.Name = $"RogueRing: marca {nombre}";
        row["iconId"]!.Value.SetValue(-1);
        row["maxHpRate"]!.Value.SetValue(1.0f);
        row["effectEndurance"]!.Value.SetValue(-1.0f);
    }
    // Esquiva del monje (tools/hks/rogue.hks): dura 0,7 s; si vuelve a esquivar antes, sale la encadenada.
    sp[7_960_003]!["effectEndurance"]!.Value.SetValue(0.7f);
    // Pasiva del tanque, Imparable: nada lo tumba ni lo hace retroceder (salvo los agarres). Sin daño
    // de postura y todo golpe cuenta como el más leve (dmgLv 8), que la postura intacta aguanta; así
    // también los de derribo, lanzamiento o empujón. Permanente (la DLL la repone si se pierde).
    {
        const int id = 79_619_040;
        var row = sp[id];
        if (row == null)
        {
            row = new Param.Row(tpl, sp) { ID = id };
            sp.InsertRow(sp.Rows.TakeWhile(x => x.ID < id).Count(), row);
        }
        row.Name = "RogueRing: imparable";
        row["iconId"]!.Value.SetValue(-1);
        row["maxHpRate"]!.Value.SetValue(1.0f);
        row["effectEndurance"]!.Value.SetValue(-1.0f);
        row["toughnessDamageCutRate"]!.Value.SetValue(0.0f);
        row["saReceiveDamageRate"]!.Value.SetValue(0.0f);
        foreach (var c in row.Cells.Where(c => c.Def.InternalName.StartsWith("dmgLv_") && c.Def.InternalName != "dmgLv_None"))
            c.Value = Convert.ChangeType(8, c.Value.GetType());
    }
    // Quemadura de las bendiciones (Ember): solo las llamas de Fire's Deadly Sin (SpEffectVfx 1790000),
    // que se ven encima de cualquier rival, sin daño ni nada más. Dura 5 s y la DLL la renueva en cada
    // quemadura; el daño lo pone el motor.
    {
        const int id = 79_619_035;
        var row = sp[id];
        if (row == null)
        {
            row = new Param.Row(tpl, sp) { ID = id };
            sp.InsertRow(sp.Rows.TakeWhile(x => x.ID < id).Count(), row);
        }
        row.Name = "RogueRing: quemado";
        row["iconId"]!.Value.SetValue(-1);
        row["maxHpRate"]!.Value.SetValue(1.0f);
        row["effectEndurance"]!.Value.SetValue(5.0f);
        row["vfxId"]!.Value.SetValue(1_790_000);
    }
    // Manos de la tierra: el aro de las manos de la Mansión Caria (SpEffectVfx 51250, el de su efecto
    // 12000 que te atrapa), solo el visual: 2 s y sin atrapar (el freno lo pone el motor).
    {
        const int id = 79_619_036;
        var row = sp[id];
        if (row == null)
        {
            row = new Param.Row(tpl, sp) { ID = id };
            sp.InsertRow(sp.Rows.TakeWhile(x => x.ID < id).Count(), row);
        }
        row.Name = "RogueRing: aro de las manos";
        row["iconId"]!.Value.SetValue(-1);
        row["maxHpRate"]!.Value.SetValue(1.0f);
        row["effectEndurance"]!.Value.SetValue(2.0f);
        row["vfxId"]!.Value.SetValue(51_250);
    }
    // Cambio de fase de un jefe: la aparición de Godfrey dorado en Leyndell (FXR 647206 del banco común,
    // el de su animación 20000; 18-10), desde el dummy 900, el del suelo, como en esa animación.
    // Dura lo que el efecto (2 s); la DLL lo renueva para alargarlo.
    {
        const int id = 79_619_041;
        var fVfx = reg.Files.First(x => x.Name.EndsWith("SpEffectVfxParam.param"));
        var vfx = LoadParam(fVfx);
        var rv = vfx[id];
        if (rv == null)
        {
            rv = new Param.Row(vfx[1_790_000]!, vfx) { ID = id };
            vfx.InsertRow(vfx.Rows.TakeWhile(x => x.ID < id).Count(), rv);
        }
        rv.Name = "RogueRing: aparición dorada";
        // Como efecto «mientras dura» y no «al empezar»: el de inicio no se apagaba nunca (en Leyndell lo corta
        // el final de la animación) y la segunda fase se quedaba dorada para siempre (18-10).
        foreach (var c in new[] { "initSfxId", "midstSeId", "initSeId", "finishSfxId", "finishSeId", "decalId1", "decalId2" })
            rv[c]!.Value.SetValue(-1);
        rv["midstSfxId"]!.Value.SetValue(647_206);
        rv["midstDmyId"]!.Value.SetValue((short)900);
        rv["initDmyId"]!.Value.SetValue((short)-1);
        fVfx.Bytes = vfx.Write();

        var row = sp[id];
        if (row == null)
        {
            row = new Param.Row(tpl, sp) { ID = id };
            sp.InsertRow(sp.Rows.TakeWhile(x => x.ID < id).Count(), row);
        }
        row.Name = "RogueRing: aparición dorada";
        row["iconId"]!.Value.SetValue(-1);
        row["maxHpRate"]!.Value.SetValue(1.0f);
        row["effectEndurance"]!.Value.SetValue(2.0f);
        row["vfxId"]!.Value.SetValue(id);
    }
    // Curar estados entre combates (19-10: la podredumbre pasaba de un combate al siguiente): los
    // efectos de cura de los bolos del juego (veneno, sangrado, congelación, sueño, podredumbre, plaga y
    // locura) sin la resistencia de después que dan los bolos (su replaceSpEffectId).
    {
        var curas = new[] { 3060, 3050, 3092, 3055, 3070, 3090, 3065 };
        for (int k = 0; k < curas.Length; k++)
        {
            int id = 79_619_043 + k;
            var orig = sp[curas[k]]!;
            var row = sp[id];
            if (row == null)
            {
                row = new Param.Row(orig, sp) { ID = id };
                sp.InsertRow(sp.Rows.TakeWhile(x => x.ID < id).Count(), row);
            }
            row.Name = $"RogueRing: cura {curas[k]}";
            row["replaceSpEffectId"]!.Value.SetValue(-1);
            row["iconId"]!.Value.SetValue(-1);
        }
    }
    // Cooperativo, caídos (dll/harness/src/director/caido.rs): no morir mientras dura el combate. La
    // opción de depuración del juego (player_no_dead) no hace nada en el juego normal (22-10: el
    // compañero murió de verdad); el campo noDead del efecto sí es del juego.
    {
        const int id = 79_619_051;
        var row = sp[id];
        if (row == null)
        {
            row = new Param.Row(tpl, sp) { ID = id };
            sp.InsertRow(sp.Rows.TakeWhile(x => x.ID < id).Count(), row);
        }
        row.Name = "RogueRing: caído, no morir";
        row["iconId"]!.Value.SetValue(-1);
        row["maxHpRate"]!.Value.SetValue(1.0f);
        row["effectEndurance"]!.Value.SetValue(-1.0f);
        var nd = row.Cells.First(c => c.Def.InternalName.Equals("noDead", StringComparison.OrdinalIgnoreCase));
        nd.Value = Convert.ChangeType(1, nd.Value.GetType());
    }
    // Caído: los enemigos no lo ven, no lo oyen y lo dejan para el final al elegir objetivo
    // (targetPriority -1, el mínimo: «en multijugador, cuánto te eligen los enemigos»; 23-10).
    {
        const int id = 79_619_052;
        var row = sp[id];
        if (row == null)
        {
            row = new Param.Row(tpl, sp) { ID = id };
            sp.InsertRow(sp.Rows.TakeWhile(x => x.ID < id).Count(), row);
        }
        row.Name = "RogueRing: caído, ignorado e intocable";
        row["iconId"]!.Value.SetValue(-1);
        row["maxHpRate"]!.Value.SetValue(1.0f);
        row["effectEndurance"]!.Value.SetValue(-1.0f);
        // Y sin recibir daño (ni de postura): también mientras se levanta y un momento después, para que
        // pueda moverse y rodar (23-10: lo remataban al levantarse).
        foreach (var col in row.Columns.Where(c => c.Def.InternalName.EndsWith("DamageCutRate") && c.ValueType == typeof(float)))
            col.SetValue(row, 0.0f);
        row["targetPriority"]!.Value.SetValue(-1.0f);
        row["sightSearchEnemyRate"]!.Value.SetValue(0.0f);
        row["hearingSearchEnemyRate"]!.Value.SetValue(0.0f);
    }
    // Sin daño de postura: la DLL se lo pone al enemigo mientras le quita la invencibilidad de sus
    // animaciones (crítico, levantarse, esquivas; juego.rs, abrir_criticos), como en Nightreign: se le
    // puede pegar, pero así no se le rompe la postura.
    {
        const int id = 79_619_055;
        var row = sp[id];
        if (row == null)
        {
            row = new Param.Row(tpl, sp) { ID = id };
            sp.InsertRow(sp.Rows.TakeWhile(x => x.ID < id).Count(), row);
        }
        row.Name = "RogueRing: sin daño de postura";
        row["iconId"]!.Value.SetValue(-1);
        row["maxHpRate"]!.Value.SetValue(1.0f);
        row["effectEndurance"]!.Value.SetValue(-1.0f);
        row["toughnessDamageCutRate"]!.Value.SetValue(0.0f);
    }
    // Marca de los proyectiles de las bendiciones: cada uno (79620000+) se la pone al rival que toca,
    // durante un momento. Así la DLL sabe que esa bajada de vida no es un golpe de tu arma
    // (motor.rs, golpe_mio), aunque el juego la apunte a tu nombre (23-10: contar golpes con el juego).
    {
        const int id = 79_619_053;
        var row = sp[id];
        if (row == null)
        {
            row = new Param.Row(tpl, sp) { ID = id };
            sp.InsertRow(sp.Rows.TakeWhile(x => x.ID < id).Count(), row);
        }
        row.Name = "RogueRing: impacto de bendición";
        row["iconId"]!.Value.SetValue(-1);
        row["maxHpRate"]!.Value.SetValue(1.0f);
        row["effectEndurance"]!.Value.SetValue(0.15f);
        // Al rival que toca el ataque (la plantilla solo afecta a uno mismo y a los aliados).
        foreach (var campo in new[] { "effectTargetEnemy", "effectTargetOpposeTarget" })
        {
            var c = row[campo]!.Value;
            c.SetValue(Convert.ChangeType(1, c.Value.GetType()));
        }
    }
    // Filas reservadas que la DLL reescribe al vuelo (bendiciones con valores que cambian en la run):
    // 79619000 daño dinámico (Trofeo, Coleccionista), 79619001 primer golpe, 79619002 Venganza.
    // Motor de efectos (dll/harness/src/motor.rs): 79619030 defensa del boss, 79619031 ataque del boss,
    // 79619032 vida máxima del jugador, 79619033 vida máxima del boss, 79619034 no morir (golpe mortal).
    foreach (var (id, nombre) in new[] { (79_619_000, "daño dinámico"), (79_619_001, "primer golpe"), (79_619_002, "venganza"), (79_619_003, "defensa dinámica"), (79_619_004, "compensación de arma"),
        (79_619_030, "boss defensa"), (79_619_031, "boss ataque"), (79_619_032, "vida jugador"), (79_619_033, "vida boss"), (79_619_034, "no morir") })
    {
        var row = sp[id];
        if (row == null)
        {
            row = new Param.Row(tpl, sp) { ID = id };
            sp.InsertRow(sp.Rows.TakeWhile(x => x.ID < id).Count(), row);
        }
        row.Name = $"RogueRing: {nombre}";
        row["maxHpRate"]!.Value.SetValue(1.0f);
        row["iconId"]!.Value.SetValue(-1);
        if (id == 79_619_034)
        {
            var nd = row.Cells.First(c => c.Def.InternalName.Equals("noDead", StringComparison.OrdinalIgnoreCase));
            nd.Value = Convert.ChangeType(1, nd.Value.GetType());
        }
    }
    f3.Bytes = sp.Write();
    Console.WriteLine($"reliquias: {creados} SpEffects");

}
// Rotura de postura del desvío (Samurai): cuando un desvío deja la postura del enemigo a 0, la DLL
// suelta este proyectil encima de él. Es la onda del Rompetierra (10410000: alcance 1,7-2,3 m, dura
// 0,2 s, solo daña a enemigos) sin efecto visual y con un ataque que apenas quita vida pero sí
// postura: el juego solo rompe la postura cuando un golpe la cruza, y así la cruza el propio desvío.
{
    const int ROTURA = 79_600_010;
    var fBul = reg.Files.First(x => x.Name.EndsWith("Bullet.param"));
    var fAtkPc = reg.Files.First(x => x.Name.EndsWith("AtkParam_Pc.param"));
    var bul = LoadParam(fBul); var atkPc = LoadParam(fAtkPc);
    var b = bul[ROTURA];
    if (b == null)
    {
        b = new Param.Row(bul[10_410_000]!, bul) { ID = ROTURA };
        bul.InsertRow(bul.Rows.TakeWhile(x => x.ID < ROTURA).Count(), b);
    }
    b.Name = "RogueRing: rotura de postura";
    b["atkId_Bullet"]!.Value.SetValue(ROTURA);
    b["sfxId_Bullet"]!.Value.SetValue(-1);
    var a = atkPc[ROTURA];
    if (a == null)
    {
        a = new Param.Row(atkPc[41_000]!, atkPc) { ID = ROTURA };
        atkPc.InsertRow(atkPc.Rows.TakeWhile(x => x.ID < ROTURA).Count(), a);
    }
    a.Name = "RogueRing: rotura de postura";
    void Set(string campo, double v) => a[campo]!.Value.SetValue(Convert.ChangeType(v, a[campo]!.Value.Value.GetType()));
    // Daño mínimo (para que cuente como golpe), sin aguante ni empuje; mucha postura. Subcategoría
    // «ataque cargado» (100): con ella el juego deja hacer el crítico a los humanoides cuya postura
    // rompe (con un golpe normal solo se tambalean).
    Set("atkMag", 1); Set("atkStam", 0); Set("knockbackDist", 0); Set("atkSuperArmor", 999);
    Set("subCategory1", 100); Set("dmgLevel", 3);
    fBul.Bytes = bul.Write(); fAtkPc.Bytes = atkPc.Write();
    Console.WriteLine("rotura de postura: proyectil y ataque 79600010");
}
// Proyectiles de las bendiciones (data/proyectiles.json): copias solo visuales de los del juego. Sin
// ataque ni en el proyectil ni en lo que suelta al chocar (la explosión va a id + 1): el daño lo pone
// el motor de la DLL, y si pegaran de verdad su golpe contaría como uno tuyo (Flurry en cadena).
{
    var fBul = reg.Files.First(x => x.Name.EndsWith("Bullet.param"));
    var bul = LoadParam(fBul);
    Param.Row Copia(int de, int id)
    {
        var b = bul[id];
        if (b == null)
        {
            b = new Param.Row(bul[de]!, bul) { ID = id };
            bul.InsertRow(bul.Rows.TakeWhile(x => x.ID < id).Count(), b);
        }
        b["atkId_Bullet"]!.Value.SetValue(0);
        // Ni los efectos que pone al chocar (la niebla helada acumula congelación, por ejemplo).
        foreach (var campo in new[] { "spEffectId0", "spEffectId1", "spEffectId2", "spEffectId3", "spEffectId4", "spEffectIDForShooter" })
            b[campo]!.Value.SetValue(-1);
        return b;
    }
    // La cadena entera: lo que suelta al chocar (HitBulletID) y lo que va creando (intervalCreateBulletId),
    // y lo que suelten esos, todo copiado sin ataque. Un hechizo como el Golpe de rayo encadena cinco
    // proyectiles y el Trío de hojas de cristal se llama a sí mismo: cada original se copia una vez y
    // las copias se apuntan entre sí. Las piezas van a 79605000 en adelante.
    int siguiente = 79_605_000;
    void CopiaArbol(int de, int id, Dictionary<int, int> hechos, string nombre)
    {
        var b = Copia(de, id);
        b.Name = $"RogueRing: {nombre}";
        hechos[de] = id;
        foreach (var campo in new[] { "HitBulletID", "intervalCreateBulletId" })
        {
            int hijo = Convert.ToInt32(bul[de]![campo]!.Value.Value);
            if (hijo <= 0 || bul[hijo] == null)
                continue;
            if (!hechos.TryGetValue(hijo, out var nuevo))
            {
                nuevo = siguiente++;
                CopiaArbol(hijo, nuevo, hechos, nombre + " (pieza)");
            }
            b[campo]!.Value.SetValue(Convert.ChangeType(nuevo, b[campo]!.Value.Value.GetType()));
        }
    }
    var lista = System.Text.Json.Nodes.JsonNode.Parse(File.ReadAllText(Path.Combine(root, "data", "proyectiles.json")))!["proyectiles"]!.AsArray();
    foreach (var p in lista)
    {
        int id = (int)p!["id"]!, de = (int)p["de"]!;
        var hechos = new Dictionary<int, int>();
        CopiaArbol(de, id, hechos, (string)p["nombre"]!);
        var b = bul[id]!;
        int hijo = Convert.ToInt32(bul[de]!["HitBulletID"]!.Value.Value);
        // sin_choque: no suelta nada al chocar.
        if ((bool?)p["sin_choque"] == true)
            b["HitBulletID"]!.Value.SetValue(-1);
        // vida_choque: lo que suelta al chocar dura menos (la nube de la niebla helada, 15 s).
        else if (hijo > 0 && hechos.TryGetValue(hijo, out var hId) && p["vida_choque"] is { } vc)
        {
            var h = bul[hId]!;
            h["life"]!.Value.SetValue(Convert.ChangeType((double)vc, h["life"]!.Value.Value.GetType()));
        }
        if ((bool?)p["sin_caida"] == true)
        {
            void Set(string campo, double v) => b[campo]!.Value.SetValue(Convert.ChangeType(v, b[campo]!.Value.Value.GetType()));
            Set("gravityInRange", 0); Set("gravityOutRange", 0); Set("dist", 40); Set("life", 2.5); Set("homingAngle", 30);
        }
    }
    // Copias con daño de verdad (23-10: «usar toda la lógica del juego»). Banco, 23-10: el daño de
    // un proyectil del jugador sale del daño fijo del ataque de la pieza que pega (en la bola de fuego,
    // la explosión: la bola no hace nada), si sale con un hechizo como origen; cambiarlo con el juego en
    // marcha no surte efecto. Así que de cada proyectil va la cadena entera del hechizo original (lo
    // que suelta al chocar y lo que va creando, con sus efectos: la congelación de la púa...) en
    // ESCALONES copias, con el daño fijo de cada pieza multiplicado por FACTOR0 * RAZON^k. La primera
    // pieza vuela como la copia visual (sin_caida). La DLL elige el escalón (motor.rs, balas reales):
    // el juego pone la defensa, la absorción de cada elemento, los números y la barra.
    // data/balas_reales.json (lo escribe esto): por proyectil visual, el primer id, el paso entre
    // escalones y el daño fijo de referencia (la suma de sus piezas, x1).
    var fAtkPc = reg.Files.First(x => x.Name.EndsWith("AtkParam_Pc.param"));
    var atkPc = LoadParam(fAtkPc);
    const int ESCALONES = 30, PASO = 20;
    const double FACTOR0 = 0.2, RAZON = 1.22;
    var mapa = new System.Text.Json.Nodes.JsonObject();
    int bloque = 79_620_000;
    foreach (var p in lista)
    {
        int id = (int)p!["id"]!, de = (int)p["de"]!;
        int primero = bloque;
        bloque += ESCALONES * PASO;
        double referencia = 0;
        for (int k = 0; k < ESCALONES; k++)
        {
            double factor = FACTOR0 * Math.Pow(RAZON, k);
            int siguienteId = primero + k * PASO;
            var hechos = new Dictionary<int, int>();
            int Real(int origen)
            {
                if (hechos.TryGetValue(origen, out var ya)) return ya;
                int nid = siguienteId++;
                if (nid >= primero + (k + 1) * PASO) throw new Exception($"cadena de {de} con más de {PASO} piezas");
                hechos[origen] = nid;
                var r = bul[nid];
                if (r == null)
                {
                    r = new Param.Row(bul[origen]!, bul) { ID = nid };
                    bul.InsertRow(bul.Rows.TakeWhile(x => x.ID < nid).Count(), r);
                }
                r.Name = $"RogueRing: {(string)p["nombre"]!} (daño, escalón {k})";
                int atk = Convert.ToInt32(bul[origen]!["atkId_Bullet"]!.Value.Value);
                if (atk > 0 && atkPc[atk] != null)
                {
                    var a = atkPc[nid];
                    if (a == null)
                    {
                        a = new Param.Row(atkPc[atk]!, atkPc) { ID = nid };
                        atkPc.InsertRow(atkPc.Rows.TakeWhile(x => x.ID < nid).Count(), a);
                    }
                    a.Name = r.Name;
                    // fisico: todo su daño, físico (el golpe invisible: nada de magia por un golpe de bendición).
                    bool fisico = (bool?)p["fisico"] == true;
                    double total = new[] { "atkPhys", "atkMag", "atkFire", "atkThun", "atkDark" }.Sum(t => Convert.ToDouble(atkPc[atk]![t]!.Value.Value));
                    foreach (var t in new[] { "atkPhys", "atkMag", "atkFire", "atkThun", "atkDark" })
                    {
                        double v = fisico ? (t == "atkPhys" ? total : 0) : Convert.ToDouble(atkPc[atk]![t]!.Value.Value);
                        if (k == 0) referencia += v;
                        a[t]!.Value.SetValue(Convert.ChangeType(Math.Min(65535, Math.Round(v * factor)), a[t]!.Value.Value.GetType()));
                    }
                    r["atkId_Bullet"]!.Value.SetValue(Convert.ChangeType(nid, r["atkId_Bullet"]!.Value.Value.GetType()));
                    // La marca de impacto de bendición (79619053) en el primer hueco de efectos libre del
                    // proyectil (los del ataque no se aplican con proyectiles: banco, 23-10).
                    int hueco = Enumerable.Range(0, 5).FirstOrDefault(i => Convert.ToInt32(r[$"spEffectId{i}"]!.Value.Value) <= 0, -1);
                    if (hueco < 0) throw new Exception($"proyectil {nid}: sin hueco para la marca de impacto");
                    var celda = r[$"spEffectId{hueco}"]!.Value;
                    celda.SetValue(Convert.ChangeType(79_619_053, celda.Value.GetType()));
                }
                foreach (var campo in new[] { "HitBulletID", "intervalCreateBulletId" })
                {
                    int hijo = Convert.ToInt32(bul[origen]![campo]!.Value.Value);
                    if (hijo > 0 && bul[hijo] != null)
                        r[campo]!.Value.SetValue(Convert.ChangeType(Real(hijo), r[campo]!.Value.Value.GetType()));
                }
                // invisible: sin efectos visuales ni de impacto.
                if ((bool?)p["invisible"] == true)
                    foreach (var campo in new[] { "sfxId_Bullet", "sfxId_Hit", "sfxId_Flick" })
                        r[campo]!.Value.SetValue(Convert.ChangeType(-1, r[campo]!.Value.Value.GetType()));
                return nid;
            }
            int raiz = Real(de);
            // La primera pieza vuela como la copia visual (sin caída, más alcance y guiado).
            if ((bool?)p["sin_caida"] == true)
                foreach (var campo in new[] { "gravityInRange", "gravityOutRange", "dist", "life", "homingAngle" })
                    bul[raiz]![campo]!.Value.SetValue(bul[id]![campo]!.Value.Value);
        }
        mapa[id.ToString()] = new System.Text.Json.Nodes.JsonObject
        {
            ["primero"] = primero, ["paso"] = PASO, ["escalones"] = ESCALONES, ["factor0"] = FACTOR0, ["razon"] = RAZON, ["referencia"] = referencia,
        };
        if (referencia <= 0) Console.WriteLine($"proyectiles: AVISO, {(string)p["nombre"]!} ({de}) no tiene daño fijo en su cadena");
    }
    File.WriteAllText(Path.Combine(root, "data", "balas_reales.json"), new System.Text.Json.Nodes.JsonObject
    {
        ["_nota"] = "Lo escribe tools/ModBuilder (proyectiles con daño de verdad, en escalones). No editar a mano.",
        ["origen_hechizo"] = 4300,
        ["balas"] = mapa,
    }.ToJsonString(new JsonSerializerOptions { WriteIndented = true }) + "\n");
    fBul.Bytes = bul.Write();
    fAtkPc.Bytes = atkPc.Write();
    Console.WriteLine($"proyectiles de las bendiciones: {lista.Count}, y su daño de verdad en {ESCALONES} escalones (79620000+)");
}
PortadorNR.Parametros(root, reg, LoadParam, jefesNr, aparienciasNr);
// Un solo bando para los enemigos de los combates (data/bandos.json): los del bando 7 y otros se pegaban
// entre ellos en las parejas del cooperativo (24-10). Solo las filas de los combates que no usa también un
// aliado, una ceniza o una bendición.
{
    var bandos = JsonDocument.Parse(File.ReadAllText(Path.Combine(root, "data", "bandos.json"))).RootElement;
    var enemigo = (byte)bandos.GetProperty("enemigo").GetInt32();
    var cambiar = bandos.GetProperty("a_enemigo").EnumerateArray().Select(v => v.GetInt32()).ToHashSet();
    var deCombate = new HashSet<int>();
    foreach (var f in Directory.GetFiles(Path.Combine(root, "data", "acts"), "*.json").Concat(Directory.GetFiles(Path.Combine(root, "data", "coop"), "*.json")))
        foreach (System.Text.RegularExpressions.Match m in System.Text.RegularExpressions.Regex.Matches(File.ReadAllText(f), @"""npc""\s*:\s*(\d+)"))
            deCombate.Add(int.Parse(m.Groups[1].Value));
    var aliados = new HashSet<int>();
    foreach (var f in new[] { "objetos/pool.json", "personajes.json", "reliquias.json" })
    {
        var texto = File.ReadAllText(Path.Combine(root, "data", f));
        foreach (System.Text.RegularExpressions.Match m in System.Text.RegularExpressions.Regex.Matches(texto, @"""npc""\s*:\s*(\d+)"))
            aliados.Add(int.Parse(m.Groups[1].Value));
        foreach (System.Text.RegularExpressions.Match m in System.Text.RegularExpressions.Regex.Matches(texto, @"\[\s*\d+\s*,\s*(\d+)\s*,\s*\d+\s*\]"))
            aliados.Add(int.Parse(m.Groups[1].Value));
    }
    var fN = reg.Files.First(x => Path.GetFileName(x.Name) == "NpcParam.param");
    var np = LoadParam(fN);
    int cambiadas = 0;
    var compartidas = new List<int>();
    foreach (var id in deCombate)
    {
        var row = np[id];
        if (row == null || !cambiar.Contains(Convert.ToInt32(row["teamType"]!.Value.Value))) continue;
        if (aliados.Contains(id)) { compartidas.Add(id); continue; }
        row["teamType"]!.Value.SetValue(enemigo);
        cambiadas++;
    }
    fN.Bytes = np.Write();
    Console.WriteLine($"bandos: {cambiadas} filas de NpcParam de combate al bando {enemigo}{(compartidas.Count > 0 ? $"; AVISO, compartidas con aliados (sin tocar): {string.Join(",", compartidas)}" : "")}");
}
// Cooperativo: las invocaciones de un compañero en el juego del anfitrión pelean por su cuenta
// (dll/harness/src/director/grupo.rs, sin_companero): copia de cada IA de compañero (isBuddyAI) y de las
// que usan las cenizas y las bendiciones del mod, en id + 1000000000, sin modo compañero.
{
    var fT = reg.Files.First(x => x.Name.EndsWith("NpcThinkParam.param"));
    var th = LoadParam(fT);
    var usadas = new HashSet<int>();
    foreach (var f in new[] { "objetos/pool.json", "personajes.json", "reliquias.json" })
    {
        var texto = File.ReadAllText(Path.Combine(root, "data", f));
        foreach (System.Text.RegularExpressions.Match m in System.Text.RegularExpressions.Regex.Matches(texto, "\"think\"\\s*:\\s*(\\d+)"))
            usadas.Add(int.Parse(m.Groups[1].Value));
        // Bendiciones con aliados: "invocar": [[chr, npc, think], ...]
        if (f == "reliquias.json")
        foreach (System.Text.RegularExpressions.Match m in System.Text.RegularExpressions.Regex.Matches(texto, "\\[\\s*\\d+\\s*,\\s*\\d+\\s*,\\s*(\\d+)\\s*\\]"))
            usadas.Add(int.Parse(m.Groups[1].Value));
    }
    int copias = 0, deCompanero = 0;
    foreach (var orig in th.Rows.ToList())
    {
        var buddy = Convert.ToInt32(orig["isBuddyAI"]!.Value.Value) != 0;
        if (orig.ID >= 1_000_000_000 || !(buddy || usadas.Contains(orig.ID))) continue;
        int id = orig.ID + 1_000_000_000;
        var row = th[id];
        if (row == null)
        {
            row = new Param.Row(orig, th) { ID = id };
            th.InsertRow(th.Rows.TakeWhile(x => x.ID < id).Count(), row);
        }
        row.Name = $"RogueRing: {orig.ID} sin modo compañero";
        row["isBuddyAI"]!.Value.SetValue((byte)0);
        copias++;
        if (buddy) deCompanero++;
    }
    var sinFila = usadas.Where(u => th[u] == null).ToList();
    fT.Bytes = th.Write();
    Console.WriteLine($"IA sin modo compañero: {copias} copias ({deCompanero} de compañero){(sinFila.Count > 0 ? $"; AVISO, sin fila: {string.Join(",", sinFila)}" : "")}");
}

var regOut = Path.Combine(root, "mod", "regulation.bin");
SFUtil.EncryptERRegulation(regOut, reg);
Console.WriteLine($"escrito {regOut}");

// Coliseo de Limgrave: su punto de aparición no tiene entidad y WarpPlayer la necesita. Se le pone
// una propia (45000980) en una copia del MSB dentro del mod.
{
    var srcMsb = Path.Combine(root, "extract", "map", "mapstudio", "m45_00_00_00.msb.dcx");
    var msb = MSBE.Read(srcMsb);
    var pl = msb.Parts.Players.First();
    pl.EntityID = 45000980;
    // Borde sur mirando al norte (rotación 180, como las marcas de invasión de ese lado): el boss
    // se crea 15 m en +Z, delante del jugador y de la cámara.
    pl.Position = new System.Numerics.Vector3(0, 0, -10);
    pl.Rotation = new System.Numerics.Vector3(0, 180, 0);
    var dstMsb = Path.Combine(root, "mod", "map", "mapstudio", "m45_00_00_00.msb.dcx");
    Directory.CreateDirectory(Path.GetDirectoryName(dstMsb)!);
    msb.Write(dstMsb, msb.Compression);
    Console.WriteLine($"coliseo: aparición con entidad {pl.EntityID} en {pl.Position}");
}


// Un coliseo por combate: una variante por encuentro con jefe (mod/map/rogue_coliseo/<id>.msb.dcx) y,
// en el evento del mapa del coliseo, el jefe colocado apagado hasta que la DLL enciende su flag.
{
    const uint GRUPO_OLA = 45000900;       // grupo de entidades de la oleada w: + w
    const uint FLAG_OLA = 1049300110;      // la DLL enciende el de la oleada w (+ w) cuando le toca
    const int OLEADAS = 5;
    const int HUECOS = 10;                 // entidades 45000990-45000999
    string[] SIN_VARIANTE = { "16000860" }; // Virgen raptora con guadañas pendulares
    var dir = Path.Combine(root, "mod", "map", "rogue_coliseo");
    Directory.CreateDirectory(dir);
    // El coliseo normal, para volver a él cuando un combate no use jefe colocado.
    File.Copy(Path.Combine(root, "mod", "map", "mapstudio", "m45_00_00_00.msb.dcx"), Path.Combine(dir, "base.msb.dcx"), true);
    // Variantes ya hechas con los mismos enemigos y el mismo coliseo y datos: no se rehacen (con las parejas
    // posibles del cooperativo son más de 2000). La clave: el coliseo base, los params y los enemigos.
    const string VERSION_VARIANTE = "1";
    var fClaves = Path.Combine(dir, "_claves.json");
    var claves = File.Exists(fClaves) ? JsonSerializer.Deserialize<Dictionary<string, string>>(File.ReadAllText(fClaves)) ?? new() : new();
    var claveBase = VERSION_VARIANTE + ":" + Convert.ToHexString(System.Security.Cryptography.SHA1.HashData(File.ReadAllBytes(Path.Combine(dir, "base.msb.dcx"))))
        + ":" + new FileInfo(Path.Combine(root, "extract", "regulation.bin")).Length + ":" + File.GetLastWriteTimeUtc(Path.Combine(root, "extract", "regulation.bin")).Ticks;
    int reusadas = 0;
    int n = 0, grupos = 0;
    // Los actos de una run en solitario y las listas del cooperativo (data/coop).
    var listas = Directory.GetFiles(Path.Combine(root, "data", "acts"), "*.json")
        .Concat(Directory.Exists(Path.Combine(root, "data", "coop")) ? Directory.GetFiles(Path.Combine(root, "data", "coop"), "*.json") : Array.Empty<string>());
    var hechos = new HashSet<string>();
    // Arenas grandes: sus variantes van a mod/map/rogue_arenas/<clave>/.
    var grandes = new Dictionary<string, ArenaGrande>();
    var fGrandes = Path.Combine(root, "data", "arenas_grandes.json");
    if (File.Exists(fGrandes))
        foreach (var x in JsonDocument.Parse(File.ReadAllText(fGrandes)).RootElement.GetProperty("arenas").EnumerateArray())
            grandes[x.GetProperty("clave").GetString()!] = new ArenaGrande(x.GetProperty("clave").GetString()!,
                x.GetProperty("map").EnumerateArray().Select(v => v.GetInt32()).ToArray(), x.GetProperty("spawn").GetUInt32(),
                V3(x.GetProperty("aparicion")), V3(x.GetProperty("centro")), x.TryGetProperty("sala", out var sl) ? sl.GetUInt32() : 0);
    var dirArenas = Path.Combine(root, "mod", "map", "rogue_arenas");
    Directory.CreateDirectory(dirArenas);
    // Como las del coliseo: no se rehacen las que tienen los mismos enemigos (con las parejas del
    // cooperativo clasificadas por tamaño, cerca de mil; tools/clasificar_arenas.py).
    var fClavesArena = Path.Combine(dirArenas, "_claves.json");
    var clavesArena = File.Exists(fClavesArena) ? JsonSerializer.Deserialize<Dictionary<string, string>>(File.ReadAllText(fClavesArena)) ?? new() : new();
    var hechasArena = new HashSet<string>();
    int reusadasArena = 0;
    var enArena = new Dictionary<string, int>();
    foreach (var f in listas.OrderBy(f => f))
    {
        foreach (var e in JsonDocument.Parse(File.ReadAllText(f)).RootElement.GetProperty("encounters").EnumerateArray())
        {
            var id = e.GetProperty("id").GetString()!;
            var ens = EnemigosDe(e);
            if (ens.Count == 0 || !hechos.Add(id)) continue;
            if (e.TryGetProperty("arena", out var ar) && ar.ValueKind == JsonValueKind.String)
            {
                if (!grandes.TryGetValue(ar.GetString()!, out var ag))
                {
                    Console.WriteLine($"AVISO arenas: {id} pide la arena {ar.GetString()}, que no está en data/arenas_grandes.json");
                    continue;
                }
                var salidaArena = Path.Combine(dirArenas, ag.Clave, $"{id}.msb.dcx");
                var claveArena = "1:" + ag + ":" + new FileInfo(Path.Combine(root, "extract", "regulation.bin")).Length + ":" + string.Join(";", ens);
                hechasArena.Add(Path.GetFullPath(salidaArena));
                if (clavesArena.GetValueOrDefault(ag.Clave + "/" + id) == claveArena && File.Exists(salidaArena)) reusadasArena++;
                else
                {
                    VarianteArena(ag, ens, salidaArena);
                    clavesArena[ag.Clave + "/" + id] = claveArena;
                }
                enArena[ag.Clave] = enArena.GetValueOrDefault(ag.Clave) + 1;
                continue;
            }
            // Colocada en el MSB no se ve ni se mueve (tools/validar_msb.py); sin variante, el director
            // la crea con spawn_chr como antes.
            if (SIN_VARIANTE.Contains(id)) continue;
            if (ens.Count > HUECOS || ens.Any(x => x.ola >= OLEADAS))
            {
                Console.WriteLine($"AVISO coliseo: {id} tiene más de {HUECOS} enemigos o {OLEADAS} oleadas; sin variante");
                continue;
            }
            var salida = Path.Combine(dir, $"{id}.msb.dcx");
            var clave = claveBase + ":" + string.Join(";", ens);
            if (claves.GetValueOrDefault(id) == clave && File.Exists(salida)) reusadas++;
            else
            {
                VarianteColiseo(ens, salida);
                claves[id] = clave;
            }
            n++;
            if (ens.Count > 1) grupos++;
        }
    }
    // Fuera las de encuentros que ya no están.
    foreach (var viejo in Directory.GetFiles(dir, "*.msb.dcx"))
    {
        var id = Path.GetFileName(viejo)[..^".msb.dcx".Length];
        if (id != "base" && !hechos.Contains(id)) { File.Delete(viejo); claves.Remove(id); }
    }
    File.WriteAllText(fClaves, JsonSerializer.Serialize(claves));
    Console.WriteLine($"coliseo: {n} variantes con los enemigos colocados ({grupos} de grupo; {reusadas} ya hechas)");

    var mapa = EMEVD.Read(Path.Combine(root, "extract", "event", "m45_00_00_00.emevd.dcx"));
    var ini = mapa.Events.First(e => e.ID == 0);
    for (int w = 0; w < OLEADAS; w++)
    {
        // Restart: se repite en cada carga del coliseo (con Default el juego lo daba por terminado tras la
        // primera visita). Este apagado no llega a frenar a los enemigos (medido: andaban 3 s antes de su
        // oleada); quien los tiene apagados de verdad es la DLL (set_chr_disabled). El evento queda para
        // encender el grupo con el flag.
        var ev = new EMEVD.Event(79_609_000 + w, EMEVD.Event.RestBehaviorType.Restart);
        // Change Character Enable State (grupo de la oleada, 0 = apagado)
        ev.Instructions.Add(Ins(2004, 5, GRUPO_OLA + (uint)w, (byte)0));
        // IF Event Flag (principal, ON, tipo flag, id)
        ev.Instructions.Add(Ins(3, 0, (sbyte)0, (byte)1, (byte)0, FLAG_OLA + (uint)w));
        ev.Instructions.Add(Ins(2004, 5, GRUPO_OLA + (uint)w, (byte)1));
        // Set Character AI State (grupo, encendida): como hacen los eventos de Nightreign al empezar una
        // pelea. Artorias se quedaba quieto en el coliseo (su IA empieza apagada); al resto no le cambia
        // nada (18-10).
        ev.Instructions.Add(Ins(2004, 1, GRUPO_OLA + (uint)w, (byte)1));
        // Set Special Standby Ended Flag (grupo, sí): saca de su espera especial a los que empiezan en
        // ella, como hacen los eventos comunes de Nightreign (Artorias se quedaba quieto en su 40000).
        ev.Instructions.Add(Ins(2004, 83, GRUPO_OLA + (uint)w, (byte)1));
        mapa.Events.Add(ev);
        // Al principio del constructor del mapa: el evento 0 salta según el tipo de partida de arena y
        // al final no se llega.
        ini.Instructions.Insert(w, Ins(2000, 0, w, (uint)(79_609_000 + w), 0u));
    }
    // Set Player Respawn Point (la llegada): quien muere en el coliseo reaparece aquí y no en su última
    // gracia (19-10: el invitado caído reaparecía en la de Radahn y no podía mirar el combate).
    ini.Instructions.Insert(0, Ins(2003, 23, 45_000_980u));
    var reacciones = Reacciones(root, mapa, i => ini.Instructions.Insert(0, i));
    Console.WriteLine($"coliseo: {reacciones} reacciones de jefes (data/reacciones.json)");
    mapa.Write(Path.Combine(root, "mod", "event", "m45_00_00_00.emevd.dcx"));
    Console.WriteLine($"coliseo: {OLEADAS} eventos de oleada (grupo {GRUPO_OLA}+w, flag {FLAG_OLA}+w)");

    // Arenas grandes: su evento de mapa es solo el de las oleadas (sin los del jefe original: escenas,
    // nieblas, barra y premio). Fuera los de mapas que ya no son arena (salvo el del coliseo).
    foreach (var viejo in Directory.GetFiles(Path.Combine(root, "mod", "event"), "m*.emevd.dcx"))
        if (!Path.GetFileName(viejo).StartsWith("m45_00_00_00") && !grandes.Values.Any(g => Path.GetFileName(viejo).StartsWith(g.Nombre)))
            File.Delete(viejo);
    foreach (var viejo in Directory.GetFiles(Path.Combine(root, "mod", "map", "mapstudio"), "m*.msb.dcx"))
        if (!Path.GetFileName(viejo).StartsWith("m45_00_00_00") && !grandes.Values.Any(g => Path.GetFileName(viejo).StartsWith(g.Nombre)))
            File.Delete(viejo);
    foreach (var ag in grandes.Values)
    {
        var dstArena = Path.Combine(root, "mod", "event", ag.Nombre + ".emevd.dcx");
        if (!enArena.ContainsKey(ag.Clave))
        {
            if (File.Exists(dstArena)) File.Delete(dstArena);
            continue;
        }
        var ev0 = EMEVD.Read(Path.Combine(root, "extract", "event", ag.Nombre + ".emevd.dcx"));
        ev0.Events.Clear();
        var cero = new EMEVD.Event(0, EMEVD.Event.RestBehaviorType.Default);
        ev0.Events.Add(cero);
        // Como en el coliseo: se reaparece en la llegada de la arena.
        cero.Instructions.Add(Ins(2003, 23, ag.Spawn));
        for (int w = 0; w < OLEADAS; w++)
        {
            var ev = new EMEVD.Event(79_609_000 + w, EMEVD.Event.RestBehaviorType.Restart);
            ev.Instructions.Add(Ins(2004, 5, GRUPO_OLA + (uint)w, (byte)0));
            ev.Instructions.Add(Ins(3, 0, (sbyte)0, (byte)1, (byte)0, FLAG_OLA + (uint)w));
            ev.Instructions.Add(Ins(2004, 5, GRUPO_OLA + (uint)w, (byte)1));
            ev.Instructions.Add(Ins(2004, 1, GRUPO_OLA + (uint)w, (byte)1));
            ev.Instructions.Add(Ins(2004, 83, GRUPO_OLA + (uint)w, (byte)1));
            ev0.Events.Add(ev);
            cero.Instructions.Add(Ins(2000, 0, w, (uint)(79_609_000 + w), 0u));
        }
        Reacciones(root, ev0, cero.Instructions.Add);
        ev0.Write(dstArena);
        // El mapa de la arena tiene que estar en el mod desde que arranca el juego: me3 solo sirve los
        // archivos que ya existen al empezar (18-10: con las variantes copiadas a mitad de partida
        // salía el mapa original). Va vacío de enemigos; la DLL pone encima la variante de cada combate.
        VarianteArena(ag, new(), Path.Combine(root, "mod", "map", "mapstudio", ag.Nombre + ".msb.dcx"));
    }
    foreach (var viejo in Directory.GetFiles(dirArenas, "*.msb.dcx", SearchOption.AllDirectories))
        if (!hechasArena.Contains(Path.GetFullPath(viejo))) File.Delete(viejo);
    File.WriteAllText(fClavesArena, JsonSerializer.Serialize(clavesArena));
    Console.WriteLine($"arenas grandes: {string.Join(", ", enArena.Select(x => $"{x.Key} {x.Value}"))} ({reusadasArena} ya hechas)");
}

// Arena grande de data/arenas_grandes.json (ver VarianteArena).
record ArenaGrande(string Clave, int[] Mapa, uint Spawn, System.Numerics.Vector3 Aparicion, System.Numerics.Vector3 Centro, uint Sala)
{
    public string Nombre => $"m{Mapa[0]:D2}_{Mapa[1]:D2}_{Mapa[2]:D2}_{Mapa[3]:D2}";
}
