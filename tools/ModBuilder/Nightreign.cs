// Jefes de Nightreign en Elden Ring (docs/JEFES_Y_MOVESETS_DE_OTROS_JUEGOS.md, data/nightreign.json).
// Nightreign usa el motor de Elden Ring y sus ids de modelo de jefe están libres en Elden Ring, así que
// modelo, texturas, animaciones (TAE y Havok 2018), comportamiento, hks, IA y sonido se copian. Lo que
// cambia se traduce aquí:
// - materiales: sus MATBIN van al allmaterial de ER, con la ruta de shader de ER; los shaders propios
//   de Nightreign se cambian por el más parecido de ER (SHADER_DE_ER);
// - efectos: el FXR de NR lleva campos de más al final de algunas acciones y se recortan en los bytes;
// - parámetros: sus filas (de los CSV de extract_nr/params) se pasan campo a campo por nombre. De ER se
//   quedan el equipo, el tipo de movimiento y lo que Nightreign no tiene (fila «plantilla»), y los
//   efectos que ya existen en ER (5400, 90000...) no se tocan.
// Fuente: extract_nr/, que saca el Extractor apuntando a la carpeta Game de Nightreign.
using System.Globalization;
using System.Text.Json;
using Andre.Formats;
using SoulsFormats;

static class PortadorNR
{
    /// SonidoExtra: bancos de sonido de otros personajes de NR que el jefe usa (el Rey sin nombre, los
    /// de su dragón c5010). ER solo carga el banco del propio jefe: se funden en el suyo.
    /// EfectosNpc: SpEffects fijos de más para un NpcParam («efectos_npc»), los que en Nightreign le pone el
    /// evento del mapa. Ahora no lo usa ninguno: a Heolstor se le puso el 46093 (un arma) en la primera fase
    /// y le salía fuego que esa fase no tiene (18-10).
    public record Jefe(int Chr, string Nombre, int[] Npc, int[] Think, int? Vida, int Plantilla, string[]? SonidoExtra = null,
        Dictionary<int, int[]>? EfectosNpc = null);

    /// Apariencia de un personaje con el cuerpo de un héroe de Nightreign (data/nightreign.json,
    /// «apariencias»): `parte` es su pieza (bd_m_5030, el cuerpo entero con cara y pelo), `id` la fila
    /// de EquipParamProtector nueva, `ropa` las cuatro piezas de ER cuyas defensas suma y `prenda` el
    /// nombre de la pieza en inglés y en español (los textos del juego, Mensajes).
    public record Apariencia(string Parte, int Id, int[] Ropa, string Nombre, string[] Prenda);

    public static List<Apariencia> LeerApariencias(string root)
    {
        var f = Path.Combine(root, "data", "nightreign.json");
        if (!File.Exists(f)) return new();
        using var doc = JsonDocument.Parse(File.ReadAllText(f));
        if (!doc.RootElement.TryGetProperty("apariencias", out var a)) return new();
        return a.EnumerateArray().Select(x => new Apariencia(
            x.GetProperty("parte").GetString()!,
            x.GetProperty("id").GetInt32(),
            x.GetProperty("ropa").EnumerateArray().Select(y => y.GetInt32()).ToArray(),
            x.GetProperty("nombre").GetString()!,
            x.GetProperty("prenda").EnumerateArray().Select(y => y.GetString()!).ToArray())).ToList();
    }

    /// Nombre y descripción de las piezas de las apariencias en los textos del juego (si no, el aviso al
    /// recibirlas y el inventario dicen «?ProtectorName?»). Van en item_dlc02.msgbnd, el que carga ER con
    /// todo lo anterior, de cada idioma extraído: español en spaes y spaar, inglés en el resto.
    public static void Mensajes(string root, List<Apariencia> apariencias)
    {
        var msg = Path.Combine(root, "extract", "msg");
        if (apariencias.Count == 0 || !Directory.Exists(msg)) return;
        int n = 0;
        foreach (var dir in Directory.GetDirectories(msg))
        {
            var src = Path.Combine(dir, "item_dlc02.msgbnd.dcx");
            if (!File.Exists(src)) continue;
            var es = Path.GetFileName(dir).StartsWith("spa") ? 1 : 0;
            string[] info = { "The look of a Nightreign hero. Keeps the defenses of your usual garb.",
                              "El aspecto de un héroe de Nightreign. Conserva las defensas de tu ropa de siempre." };
            string[] caption = { "Won by finishing a run with this character.\n\nCovers the whole body. Its defenses are those of the character's own set.",
                                 "Se gana al terminar una run con este personaje.\n\nCubre todo el cuerpo. Sus defensas son las del conjunto del personaje." };
            var bnd = BND4.Read(src);
            foreach (var f in bnd.Files)
            {
                var fmgNombre = Path.GetFileNameWithoutExtension(f.Name);
                Func<Apariencia, string>? texto = fmgNombre.StartsWith("ProtectorName") ? a => a.Prenda[es]
                    : fmgNombre.StartsWith("ProtectorInfo") ? _ => info[es]
                    : fmgNombre.StartsWith("ProtectorCaption") ? _ => caption[es]
                    : null;
                if (texto == null) continue;
                var fmg = FMG.Read(f.Bytes);
                foreach (var a in apariencias)
                {
                    fmg.Entries.RemoveAll(e => e.ID == a.Id);
                    fmg.Entries.Add(new FMG.Entry(fmg, a.Id, texto(a)));
                }
                fmg.Entries.Sort((x, y) => x.ID.CompareTo(y.ID));
                f.Bytes = fmg.Write();
            }
            var dst = Path.Combine(root, "mod", "msg", Path.GetFileName(dir), "item_dlc02.msgbnd.dcx");
            Directory.CreateDirectory(Path.GetDirectoryName(dst)!);
            bnd.Write(dst);
            n++;
        }
        Console.WriteLine($"nombres de las apariencias en {n} idiomas");
    }

    const string RUTA_NR = @"W:\CL\data\Target\INTERROOT_win64";
    const string RUTA_ER = @"N:\GR\data\INTERROOT_win64";

    /// Shaders de Nightreign que ER no tiene: el MATBIN de ER que hace de plantilla, cuando el parecido
    /// por nombre (ShaderParecido) no basta. Sus muestras se rellenan con las texturas del de NR.
    static readonly (string spx, string plantilla)[] SHADER_DE_ER =
    {
        ("Veill", "c5270[Veil]_cloth.matbin"),   // velo de la Bailarina
        // Cuerpo y cabeza del Demonio ciempiés: el parecido daba la variante [S2] de ER, que pide una
        // segunda textura de brillo que este material no trae, y el cuerpo no se dibujaba (16-10:
        // solo se le veían las patas). Con el de brillo corriente salía blanco entero (su textura de brillo
        // a tope): el de ER sin brillo, de un cuerpo (c2271).
        ("DetailBlend][1_0_0_0]_Glow[Int]", "C[c2271]_Body.matbin"),
    };

    /// Trozos del nombre de un shader: «C[DetailBlend][1_0_0_0]_Glow[MaskScroll]2_cloth» da DetailBlend,
    /// 1_0_0_0, Glow, MaskScroll, 2 y cloth. Los del modelo (c7600) no cuentan.
    static HashSet<string> Trozos(string spx)
    {
        var n = Path.GetFileNameWithoutExtension(spx);
        var t = System.Text.RegularExpressions.Regex.Matches(n, @"\[([^\]]+)\]|[A-Za-z0-9]+")
            .Select(m => m.Groups[1].Success ? m.Groups[1].Value : m.Value)
            .Where(x => x != "C" && x != "P" && !System.Text.RegularExpressions.Regex.IsMatch(x, @"^[cC]\d{4}$|^[A-Z]{2}_[A-Z]_\d{4}$"))
            .ToHashSet(StringComparer.OrdinalIgnoreCase);
        return t;
    }

    /// El shader de ER más parecido por nombre: los mismos trozos (y tela con tela). Sin nada en común,
    /// el de personaje corriente (C[DetailBlend], o su versión de tela).
    static string ShaderParecido(string spx, IEnumerable<string> deEr)
    {
        var mios = Trozos(spx);
        bool tela = mios.Contains("cloth");
        // De la misma familia (C de personaje, P de pieza de armadura) si la hay.
        var familia = spx[..1];
        var mismos = deEr.Where(e => e.StartsWith(familia + "[", StringComparison.OrdinalIgnoreCase)).ToList();
        if (mismos.Count > 0) deEr = mismos;
        // Los de un modelo concreto de ER (C[c2200]_Body) no: son de ese modelo.
        var mejor = deEr.Where(e => !System.Text.RegularExpressions.Regex.IsMatch(e, @"\[[cC]\d{4}\]|\[[A-Z]{2}_[A-Z]_\d{4}\]") && Trozos(e).Contains("cloth") == tela)
            .Select(e => (e, comun: Trozos(e).Intersect(mios, StringComparer.OrdinalIgnoreCase).Count(e2 => e2 != "cloth"), sobra: Trozos(e).Except(mios, StringComparer.OrdinalIgnoreCase).Count()))
            .Where(x => x.comun > 0)
            .OrderByDescending(x => x.comun).ThenBy(x => x.sobra).ThenBy(x => x.e.Length)
            .Select(x => x.e).FirstOrDefault();
        var base_ = $"{familia}[DetailBlend]{(tela ? "_cloth" : "")}.spx";
        return mejor ?? (deEr.Contains(base_, StringComparer.OrdinalIgnoreCase) ? base_ : tela ? "C[DetailBlend]_cloth.spx" : "C[DetailBlend].spx");
    }

    /// Papel de una muestra de textura (AlbedoMap, NormalMap...) a partir de su nombre en el shader.
    static string Papel(string tipo)
    {
        // Solo lo que va tras «_snp_» (Texture2D_7_AlbedoMap): en el nombre entero salía antes el del shader
        // («Detail» de C_DetailBlend_...) y todas las muestras de C[DetailBlend] parecían de detalle.
        if (tipo.Contains("_snp_")) tipo = tipo[(tipo.LastIndexOf("_snp_", StringComparison.Ordinal) + 5)..];
        var m = System.Text.RegularExpressions.Regex.Match(tipo, @"(Albedo|Normal|Metallic|Emissive|Mask|Specular|Shininess|Blend|Detail)\w*", System.Text.RegularExpressions.RegexOptions.IgnoreCase);
        return m.Success ? m.Groups[1].Value.ToLowerInvariant() : tipo;
    }

    /// Nombre de una textura sin su sufijo de papel: c7580_rags01_n.tif -> c7580_rags01.
    static string Base(string ruta)
    {
        if (string.IsNullOrEmpty(ruta)) return "";
        var n = Path.GetFileNameWithoutExtension(ruta);
        int i = n.LastIndexOf('_');
        return i > 0 ? n[..i].ToLowerInvariant() : n.ToLowerInvariant();
    }

    /// Textura propia del modelo (c7580_body_a...) y no una de detalle común (AAT100_Metal_02_n...).
    static bool Propia(string ruta) => !string.IsNullOrEmpty(ruta) && !ruta.StartsWith("AAT", StringComparison.OrdinalIgnoreCase)
        && !ruta.StartsWith("SYS", StringComparison.OrdinalIgnoreCase);

    /// Cuánto es «principal» cada muestra (por su nombre, que lleva el del shader): en qué parte de los
    /// materiales de un juego que la tienen lleva una textura propia del modelo. En C[DetailBlend] el
    /// albedo principal es el 7 (casi siempre c####_..._a) y el 1 es de detalle (AAT...).
    static Dictionary<string, double> Principales(params BND4[] bancos)
    {
        var total = new Dictionary<string, int>(); var propias = new Dictionary<string, int>();
        foreach (var b in bancos)
            foreach (var f in b.Files)
            {
                MATBIN m;
                try { m = MATBIN.Read(f.Bytes); } catch { continue; }
                foreach (var x in m.Samplers)
                {
                    total[x.Type] = total.GetValueOrDefault(x.Type) + 1;
                    if (Propia(x.Path)) propias[x.Type] = propias.GetValueOrDefault(x.Type) + 1;
                }
            }
        return total.ToDictionary(kv => kv.Key, kv => propias.GetValueOrDefault(kv.Key) / (double)kv.Value);
    }

    /// Un MATBIN de ER (plantilla) con las texturas del de NR, por papel (albedo, normal, metal...): las
    /// propias del modelo a las muestras principales de la plantilla y las de detalle a las demás. Antes se
    /// emparejaban por el nombre de la muestra, pero el número cambia de sentido de un shader a otro: las
    /// vendas de Heolstor acababan en el hueco de detalle del C[DetailBlend] y el principal vacío (gris).
    static MATBIN ConTexturas(MATBIN plantilla, MATBIN nr, Dictionary<string, double> principal)
    {
        var nuevo = MATBIN.Read(plantilla.Write());
        foreach (var s in nuevo.Samplers) s.Path = "";
        double P(string t) => principal.GetValueOrDefault(t);
        foreach (var papel in nuevo.Samplers.Select(x => Papel(x.Type)).Distinct().ToList())
        {
            var huecos = nuevo.Samplers.Where(x => Papel(x.Type) == papel).OrderByDescending(x => P(x.Type)).ToList();
            var deNr = nr.Samplers.Where(x => Papel(x.Type) == papel && !string.IsNullOrEmpty(x.Path)).ToList();
            // Empate entre propias: la que comparte nombre con texturas de otros papeles (rags01_a con
            // rags01_n y rags01_em) es la principal, no una suelta pequeña (rags_body_a).
            int Hermanas(string ruta) => nr.Samplers.Count(y => Papel(y.Type) != papel && Base(y.Path) == Base(ruta) && Base(ruta) != "");
            var orden = deNr.Where(x => Propia(x.Path)).OrderByDescending(x => P(x.Type)).ThenByDescending(x => Hermanas(x.Path))
                .Concat(deNr.Where(x => !Propia(x.Path))).Select(x => x.Path).ToList();
            for (int k = 0; k < Math.Min(huecos.Count, orden.Count); k++) huecos[k].Path = orden[k];
        }
        // Parámetros generales (g_AlphaRef, g_GlowScale, g_DiffuseMapColor...) los de NR si la plantilla los
        // tiene: con los de la plantilla, las espadas de Heolstor brillaban a tope (GlowScale 1 en vez de 0,3)
        // y salían blancas, y su manto recortaba con 50 en vez de 10 (18-10).
        foreach (var p in nuevo.Params.Where(p => p.Name.StartsWith("g_") && !p.Name.StartsWith("g_GX_")))
        {
            var q = nr.Params.FirstOrDefault(x => x.Name == p.Name);
            if (q != null && q.Type == p.Type) p.Value = q.Value;
        }
        nuevo.SourcePath = nr.SourcePath;
        return nuevo;
    }

    /// Texturas a mano para un material de un jefe («materiales» en data/nightreign.json, por nombre del
    /// MATBIN): {"albedo": ..., "normal": ...} en las muestras principales de ese papel. El manto de fuego
    /// de Heolstor usa en NR un shader de llamas animadas con una textura de color minúscula; con la morada
    /// de su manto (c7580_mantle_a) queda como en los mods de armadura de Heolstor.
    /// La clave es un papel (albedo, normal...) o un trozo del nombre de la muestra (AlphaNoise); «plantilla»
    /// (un MATBIN de ER) se usa antes, en Materiales.
    static void TexturasAMano(MATBIN m, JsonElement datos, Dictionary<string, double> principal)
    {
        if (datos.TryGetProperty("params", out var ps))
            foreach (var p in ps.EnumerateObject())
            {
                var par = m.Params.FirstOrDefault(x => x.Name == p.Name);
                if (par == null) continue;
                par.Value = par.Value switch
                {
                    float[] => p.Value.EnumerateArray().Select(v => v.GetSingle()).ToArray(),
                    int[] => p.Value.EnumerateArray().Select(v => v.GetInt32()).ToArray(),
                    float => p.Value.GetSingle(),
                    int => p.Value.GetInt32(),
                    bool => p.Value.GetBoolean(),
                    _ => par.Value,
                };
            }
        foreach (var p in datos.EnumerateObject().Where(p => p.Name is not "plantilla" and not "params"))
        {
            var hueco = m.Samplers.Where(x => Papel(x.Type) == p.Name || x.Type.Contains(p.Name, StringComparison.OrdinalIgnoreCase))
                .OrderByDescending(x => principal.GetValueOrDefault(x.Type)).FirstOrDefault();
            if (hueco != null) hueco.Path = p.Value.GetString()!;
        }
    }

    public static List<Jefe> Leer(string root)
    {
        var f = Path.Combine(root, "data", "nightreign.json");
        if (!File.Exists(f)) return new();
        using var doc = JsonDocument.Parse(File.ReadAllText(f));
        return doc.RootElement.GetProperty("jefes").EnumerateArray().Select(j => new Jefe(
            j.GetProperty("chr").GetInt32(),
            j.GetProperty("nombre").GetString()!,
            j.GetProperty("npc").EnumerateArray().Select(x => x.GetInt32()).ToArray(),
            j.GetProperty("think").EnumerateArray().Select(x => x.GetInt32()).ToArray(),
            j.TryGetProperty("vida", out var v) ? v.GetInt32() : null,
            j.GetProperty("plantilla").GetInt32(),
            j.TryGetProperty("sonido_extra", out var se) ? se.EnumerateArray().Select(x => x.GetString()!).ToArray() : null,
            j.TryGetProperty("efectos_npc", out var en)
                ? en.EnumerateObject().ToDictionary(x => int.Parse(x.Name), x => x.Value.EnumerateArray().Select(v => v.GetInt32()).ToArray())
                : null)).ToList();
    }

    static string Modelo(Jefe j) => $"c{j.Chr:D4}";

    static bool Disponible(string root, Jefe j)
    {
        var ok = File.Exists(Path.Combine(root, "extract_nr", "chr", Modelo(j) + ".chrbnd.dcx"));
        if (!ok)
            Console.WriteLine($"Nightreign: falta {Modelo(j)} en extract_nr (Extractor <Game de Nightreign> extract_nr chr/{Modelo(j)} " +
                $"action/script/{Modelo(j)}.hks sfx/sfxbnd_{Modelo(j)} script/<objetivo>_battle cs_{Modelo(j)}.bnk material/allmaterial)");
        return ok;
    }

    /// Copia un bnd con las rutas internas de ER y sus modelos en el FLVER de ER.
    /// `sinTela`: fuera la física de tela de Nightreign (_c.hkx). ER no la sabe simular: con ella, la capa
    /// de la Duquesa no se dibuja (probado en el banco el 23-10). Sin ella la tela va pegada al cuerpo,
    /// como en las piezas de ER que traen mallas de tela sin .hkx (bd_m_1100).
    static void CopiarBnd(string de, string a, bool sinMarcas = false, bool sinTela = false)
    {
        var b = BND4.Read(de);
        if (sinTela) b.Files.RemoveAll(f => f.Name.EndsWith("_c.hkx", StringComparison.OrdinalIgnoreCase));
        foreach (var f in b.Files)
        {
            f.Name = f.Name.Replace(RUTA_NR, RUTA_ER);
            if (f.Name.EndsWith(".flver")) f.Bytes = FlverDeNRaER(f.Bytes.ToArray(), sinMarcas);
        }
        Directory.CreateDirectory(Path.GetDirectoryName(a)!);
        b.Write(a, b.Compression);
    }

    /// Modelo de Nightreign (FLVER 0x20021) en el de ER (0x2001A): misma estructura para SoulsFormats
    /// salvo la cabecera y dos tipos de vértice que ER no conoce. Sin esto el jefe sale invisible y
    /// quieto (en su pose de referencia, la animación 9500).
    /// - Índices de hueso UShort2 (dos huesos) pasan a UShort4, que ER sí usa.
    /// - UV en Byte4 o UByte4: SoulsFormats los lee como dos short, igual que Short2; se cambia solo el tipo.
    /// MATBIN que piden los modelos convertidos (por nombre de archivo): los que ER no tenga salen de NR.
    static readonly HashSet<string> materialesUsados = new(StringComparer.OrdinalIgnoreCase);

    static byte[] FlverDeNRaER(byte[] bytes, bool sinMarcas = false)
    {
        var fl = FLVER2.Read(bytes);
        foreach (var m in fl.Materials) materialesUsados.Add(Path.GetFileNameWithoutExtension(m.MTD.Replace('\\', '/')) + ".matbin");
        if (fl.Header.Version <= 0x2001A) return bytes;
        // Apariencias: el cuerpo del héroe lleva cara y pelo marcados (#05#, #06#...) como el cuerpo base
        // del jugador; sin las marcas no se ocultan nunca, y la fila de la armadura oculta todo lo demás.
        if (sinMarcas)
            foreach (var m in fl.Materials)
                m.Name = System.Text.RegularExpressions.Regex.Replace(m.Name, @"#\d+#", "").Trim();
        fl.Header.Version = 0x2001A;
        fl.Header.Unk68 = 4;
        foreach (var l in fl.BufferLayouts)
            foreach (var m in l)
            {
                if (m.Semantic == FLVER.LayoutSemantic.BoneIndices && m.Type == FLVER.LayoutType.UShort2)
                    m.Type = FLVER.LayoutType.UShort4;
                else if (m.Semantic == FLVER.LayoutSemantic.UV && m.Type is FLVER.LayoutType.Byte4 or FLVER.LayoutType.UByte4)
                    m.Type = FLVER.LayoutType.Short2;
            }
        // Nightreign guarda cada atributo del vértice en su propio búfer; ER, todos juntos en uno. Una
        // capa por malla con los atributos de todos sus búferes, en el mismo orden (las iguales se comparten).
        var capas = new List<FLVER2.BufferLayout>();
        foreach (var mesh in fl.Meshes)
        {
            var junta = new FLVER2.BufferLayout();
            foreach (var vb in mesh.VertexBuffers)
                foreach (var m in fl.BufferLayouts[vb.LayoutIndex])
                    junta.Add(new FLVER.LayoutMember(m.Type, m.Semantic, m.Index, 0, m.SpecialModifier));
            // Todos los shaders de personaje de ER leen el color de vértice 1 (las 67 mallas de los jefes
            // de ER que se miraron lo llevan); Nightreign no lo lleva y sin él la malla no se dibuja. Blanco.
            if (!junta.Any(m => m.Semantic == FLVER.LayoutSemantic.VertexColor && m.Index == 1))
            {
                junta.Add(new FLVER.LayoutMember(FLVER.LayoutType.UByte4Norm, FLVER.LayoutSemantic.VertexColor, 1));
                foreach (var v in mesh.Vertices) v.Colors.Add(new FLVER.VertexColor((byte)255, (byte)255, (byte)255, (byte)255));
            }
            // Mallas de tela (las que llevan Position1): los shaders _cloth de ER leen también Normal1 y
            // Tangent1, que en las de ER son copia de Normal0 y Tangent0 (como Position1 de Position0, en
            // los dos juegos). Nightreign no los lleva y sin ellos la malla no se dibujaba: la capa de la
            // Duquesa, los faldones de su chaqueta, la falda de la Renacida.
            bool tela = junta.Any(m => m.Semantic == FLVER.LayoutSemantic.Position && m.Index == 1);
            if (tela && !junta.Any(m => m.Semantic == FLVER.LayoutSemantic.Normal && m.Index == 1))
                junta.Add(new FLVER.LayoutMember(FLVER.LayoutType.UByte4, FLVER.LayoutSemantic.Normal, 1));
            if (tela && !junta.Any(m => m.Semantic == FLVER.LayoutSemantic.Tangent && m.Index == 1))
            {
                // Las tangentes se escriben en el orden de la capa: la nueva, la última, va al final.
                junta.Add(new FLVER.LayoutMember(FLVER.LayoutType.UByte4, FLVER.LayoutSemantic.Tangent, 1));
                foreach (var v in mesh.Vertices) v.Tangents.Add(v.Tangents[0]);
            }
            string clave(FLVER2.BufferLayout l) => string.Join(",", l.Select(m => $"{m.Type}/{m.Semantic}/{m.Index}/{m.SpecialModifier}"));
            int i = capas.FindIndex(c => clave(c) == clave(junta));
            if (i < 0) { capas.Add(junta); i = capas.Count - 1; }
            mesh.VertexBuffers = new List<FLVER2.VertexBuffer> { new FLVER2.VertexBuffer(i) };
            // Caras: ER lleva cada nivel de detalle dos veces, la segunda para el desenfoque de movimiento
            // (salvo en las mallas de tela, que no la llevan).
            if (!tela && !mesh.FaceSets.Any(fs => fs.Flags.HasFlag(FLVER2.FaceSet.FSFlags.MotionBlur)))
                mesh.FaceSets.AddRange(mesh.FaceSets.ToList().Select(fs => new FLVER2.FaceSet(fs.Flags | FLVER2.FaceSet.FSFlags.MotionBlur, fs.TriangleStrip, fs.CullBackfaces, fs.Unk06, fs.Indices.ToList())));
        }
        fl.BufferLayouts = capas;
        // Materiales con la ruta entera, como los de ER («N:\GR\data\Material\mtd\Chr\matxml\...»).
        foreach (var mat in fl.Materials.Where(m => !m.MTD.Contains('\\')))
            mat.MTD = @"N:\GR\data\Material\mtd\Chr\matxml\" + mat.MTD;
        return fl.Write();
    }

    /// Modelo, texturas, animaciones, comportamiento, hks, IA, efectos y sonido.
    public static void Archivos(string root, List<Jefe> jefes, List<Apariencia> apariencias)
    {
        // Apariencias: la pieza del héroe (y su versión lejana _l) con el modelo de ER y sin marcas.
        foreach (var a in apariencias)
        {
            var de = Path.Combine(root, "extract_nr", "parts", a.Parte + ".partsbnd.dcx");
            if (!File.Exists(de))
            {
                Console.WriteLine($"Nightreign: falta la pieza {a.Parte} (Extractor <Game de Nightreign> extract_nr parts/{a.Parte})");
                continue;
            }
            foreach (var f in new[] { de, de.Replace(".partsbnd", "_l.partsbnd") }.Where(File.Exists))
                CopiarBnd(f, Path.Combine(root, "mod", "parts", Path.GetFileName(f)), sinMarcas: true, sinTela: true);
            Console.WriteLine($"Nightreign: apariencia {a.Nombre} ({a.Parte} -> protector {a.Id})");
        }
        var nr = Path.Combine(root, "extract_nr");
        var mod = Path.Combine(root, "mod");
        var canon = CamposFxrDeER(root);
        var efectos = new List<BinderFile>();
        foreach (var j in jefes.Where(j => Disponible(root, j)))
        {
            var c = Modelo(j);
            var copiados = new List<string>();
            foreach (var f in Directory.GetFiles(Path.Combine(nr, "chr"), c + "*.dcx"))
            {
                CopiarBnd(f, Path.Combine(mod, "chr", Path.GetFileName(f)));
                copiados.Add(Path.GetFileName(f));
            }
            // hks del personaje (bytecode de HavokScript, el mismo que el de ER).
            var hks = Path.Combine(nr, "action", "script", c + ".hks");
            if (File.Exists(hks))
            {
                Directory.CreateDirectory(Path.Combine(mod, "action", "script"));
                File.Copy(hks, Path.Combine(mod, "action", "script", c + ".hks"), true);
                copiados.Add(c + ".hks");
            }
            // IA: el objetivo de combate de su NpcThinkParam (script/<id>_battle.luabnd), con lo que en
            // Nightreign le daban sus scripts comunes (IaParaER).
            var think = Csv(root, "NpcThinkParam");
            foreach (var t in j.Think)
            {
                var goal = (int)Num(think[t]["battleGoalID"]);
                var lua = Path.Combine(nr, "script", $"{goal:D6}_battle.luabnd.dcx");
                if (File.Exists(lua))
                {
                    var b = BND4.Read(lua);
                    var notas = new List<string>();
                    foreach (var f in b.Files)
                    {
                        f.Name = f.Name.Replace(RUTA_NR, RUTA_ER);
                        if (f.Name.EndsWith(".lua")) f.Bytes = IaParaER(root, f.Bytes.ToArray(), notas);
                    }
                    Directory.CreateDirectory(Path.Combine(mod, "script"));
                    b.Write(Path.Combine(mod, "script", Path.GetFileName(lua)), b.Compression);
                    copiados.Add($"{Path.GetFileName(lua)}{(notas.Count > 0 ? " (" + string.Join("; ", notas) + ")" : "")}");
                }
                else Console.WriteLine($"Nightreign: falta la IA {Path.GetFileName(lua)} de {c}");
            }
            // Efectos del personaje, con el FXR de NR recortado al de ER. No van en su banco propio
            // (sfxbnd_c7920): el juego no carga el de un modelo que ER no tiene y salían sin efectos
            // (16-10). Van al común del DLC 2 (EfectosEnComun), que está siempre cargado.
            var sfx = Path.Combine(nr, "sfx", $"sfxbnd_{c}.ffxbnd.dcx");
            var viejo = Path.Combine(mod, "sfx", Path.GetFileName(sfx));
            if (File.Exists(viejo)) File.Delete(viejo);
            if (File.Exists(sfx) && canon != null)
            {
                var b = BND4.Read(sfx);
                foreach (var f in b.Files)
                {
                    f.Name = f.Name.Replace(RUTA_NR, RUTA_ER);
                    if (f.Name.EndsWith(".flver")) f.Bytes = FlverDeNRaER(f.Bytes.ToArray());
                    efectos.Add(f);
                }
                copiados.Add($"{Path.GetFileName(sfx)} ({b.Files.Count(f => f.Name.EndsWith(".fxr"))} FXR al común)");
            }
            // Sonido: banco Wwise (misma versión, 135, en los dos juegos). Los que hablan lo tienen en enus.
            foreach (var sub in new[] { "", "enus" })
            {
                var bnk = Path.Combine(nr, sub, $"cs_{c}.bnk");
                if (!File.Exists(bnk)) continue;
                Directory.CreateDirectory(Path.Combine(mod, "sd", sub));
                var destino = Path.Combine(mod, "sd", sub, Path.GetFileName(bnk));
                File.Copy(bnk, destino, true);
                copiados.Add(Path.Combine(sub, Path.GetFileName(bnk)));
                foreach (var extra in sub == "" ? j.SonidoExtra ?? [] : [])
                {
                    var otro = Path.Combine(nr, $"cs_{extra}.bnk");
                    if (!File.Exists(otro)) { Console.WriteLine($"Nightreign {c}: falta extract_nr/cs_{extra}.bnk (Extractor sobre Nightreign: cs_{extra}.bnk)"); continue; }
                    copiados.Add($"+{FundirBnk(destino, otro)}");
                }
                if (!ProyectoDeEr(root, destino)) Console.WriteLine($"Nightreign {c}: falta extract/sd/init.bnk; su sonido no sonará (proyecto de Wwise de NR)");
                copiados.Add(AudioPcm(root, destino));
            }
            Console.WriteLine($"Nightreign {c} ({j.Nombre}): {string.Join(", ", copiados)}");
        }
        ConvertirFxr(root, efectos.Where(f => f.Name.EndsWith(".fxr")).ToList(), canon);
        EfectosEnComun(root, efectos);
        Materiales(root);
        TablasDeNombres(root);
    }

    /// El audio del banco a PCM (tools/bnk_pcm.py con tools/vendor/vgmstream): el Vorbis de NR usa
    /// otros libros de códigos y el Wwise de ER no lo decodifica (Fundidor mudo; con PCM suena, el jugador
    /// 18-10). Sin Python o sin vgmstream, el banco se queda como está y el jefe, mudo.
    static string AudioPcm(string root, string banco)
    {
        var vgm = Path.Combine(root, "tools", "vendor", "vgmstream", "vgmstream-cli.exe");
        if (!File.Exists(vgm)) return "SIN PCM: falta tools/vendor/vgmstream (vgmstream-win64 de GitHub)";
        try
        {
            var psi = new System.Diagnostics.ProcessStartInfo("python", $"\"{Path.Combine(root, "tools", "bnk_pcm.py")}\" \"{banco}\" \"{banco}\" \"{vgm}\"")
                { RedirectStandardOutput = true, RedirectStandardError = true, UseShellExecute = false };
            using var pr = System.Diagnostics.Process.Start(psi)!;
            var salida = pr.StandardOutput.ReadToEnd().Trim();
            var error = pr.StandardError.ReadToEnd().Trim();
            pr.WaitForExit();
            return pr.ExitCode == 0 ? salida.Split((char)10).Last() : "SIN PCM: " + error.Split((char)10).Last();
        }
        catch (Exception e) { return "SIN PCM: " + e.Message; }
    }

    /// Pone en el banco Wwise `destino` el proyecto de Wwise de ER (BKHD, dwProjectID: 0x772 en ER,
    /// 0x31bc en NR). Con el de NR el juego abría el banco y no sonaba nada del jefe: Wwise no acepta
    /// bancos de otro proyecto que el de su init.bnk (Fundidor mudo, 18-10). Se lee de init.bnk.
    static bool ProyectoDeEr(string root, string destino)
    {
        var init = Path.Combine(root, "extract", "sd", "init.bnk");
        if (!File.Exists(init)) return false;
        var i = File.ReadAllBytes(init);
        var b = File.ReadAllBytes(destino);
        if (System.Text.Encoding.ASCII.GetString(i, 0, 4) != "BKHD" || System.Text.Encoding.ASCII.GetString(b, 0, 4) != "BKHD") return false;
        Array.Copy(i, 8 + 16, b, 8 + 16, 4);
        File.WriteAllBytes(destino, b);
        return true;
    }

    /// Mete en el banco Wwise `destino` los objetos (HIRC) y el audio (DIDX/DATA) de `otro` que no
    /// tenga ya (misma versión, 135). Los eventos de los dos quedan en el banco que ER carga.
    static string FundirBnk(string destino, string otro)
    {
        static List<(string tag, byte[] datos)> Secciones(byte[] b)
        {
            var r = new List<(string, byte[])>();
            for (int i = 0; i + 8 <= b.Length;)
            {
                int n = BitConverter.ToInt32(b, i + 4);
                r.Add((System.Text.Encoding.ASCII.GetString(b, i, 4), b.AsSpan(i + 8, n).ToArray()));
                i += 8 + n;
            }
            return r;
        }
        static List<(uint id, byte[] obj)> Objetos(byte[] h)
        {
            var r = new List<(uint, byte[])>();
            int c = BitConverter.ToInt32(h, 0), j = 4;
            for (int k = 0; k < c; k++)
            {
                int ln = BitConverter.ToInt32(h, j + 1);
                r.Add((BitConverter.ToUInt32(h, j + 5), h.AsSpan(j, 5 + ln).ToArray()));
                j += 5 + ln;
            }
            return r;
        }
        var a = Secciones(File.ReadAllBytes(destino));
        var b = Secciones(File.ReadAllBytes(otro)).ToDictionary(x => x.tag, x => x.datos);
        byte[] Sec(string t) => a.First(x => x.tag == t).datos;
        // Audio: el de `otro` detrás del de `destino`, alineado a 16.
        var didx = new List<(uint id, int off, int len)>();
        var da = Sec("DIDX");
        for (int k = 0; k < da.Length / 12; k++) didx.Add((BitConverter.ToUInt32(da, k * 12), BitConverter.ToInt32(da, k * 12 + 4), BitConverter.ToInt32(da, k * 12 + 8)));
        var data = new List<byte>(Sec("DATA"));
        var ya = didx.Select(x => x.id).ToHashSet();
        int medios = 0;
        for (int k = 0; k < b["DIDX"].Length / 12; k++)
        {
            uint id = BitConverter.ToUInt32(b["DIDX"], k * 12);
            int off = BitConverter.ToInt32(b["DIDX"], k * 12 + 4), len = BitConverter.ToInt32(b["DIDX"], k * 12 + 8);
            if (!ya.Add(id)) continue;
            while (data.Count % 16 != 0) data.Add(0);
            didx.Add((id, data.Count, len));
            data.AddRange(b["DATA"].AsSpan(off, len).ToArray());
            medios++;
        }
        var objs = Objetos(Sec("HIRC"));
        var ids = objs.Select(o => o.id).ToHashSet();
        int nuevos = 0;
        foreach (var o in Objetos(b["HIRC"]))
            if (ids.Add(o.id)) { objs.Add(o); nuevos++; }
        var hirc = new List<byte>(BitConverter.GetBytes(objs.Count));
        foreach (var o in objs) hirc.AddRange(o.obj);
        var nuevoDidx = didx.SelectMany(x => BitConverter.GetBytes(x.id).Concat(BitConverter.GetBytes(x.off)).Concat(BitConverter.GetBytes(x.len))).ToArray();
        using var fs = File.Create(destino);
        foreach (var (tag, datos) in a)
        {
            var d = tag switch { "DIDX" => nuevoDidx, "DATA" => data.ToArray(), "HIRC" => hirc.ToArray(), _ => datos };
            fs.Write(System.Text.Encoding.ASCII.GetBytes(tag));
            fs.Write(BitConverter.GetBytes(d.Length));
            fs.Write(d);
        }
        return $"{Path.GetFileName(otro)} ({nuevos} objetos, {medios} audios)";
    }

    /// Los grafos de comportamiento de Havok (behbnd, con su c9997 común) se enlazan con el juego por
    /// nombre de evento, estado y variable, con las tablas globales de action/. Nightreign tiene 476
    /// eventos, 465 estados y 157 variables que ER no conoce: se añaden al final de las tablas de ER con
    /// ids nuevos (los de ER no cambian).
    static void TablasDeNombres(string root)
    {
        var notas = new List<string>();
        foreach (var n in new[] { "eventnameid", "statenameid", "variablenameid" })
        {
            var er = Path.Combine(root, "extract", "action", n + ".txt");
            var nr = Path.Combine(root, "extract_nr", "action", n + ".txt");
            if (!File.Exists(er) || !File.Exists(nr)) { notas.Add($"falta {n}.txt"); continue; }
            var bytes = File.ReadAllBytes(er);
            int fin = bytes.Length;
            while (fin > 0 && bytes[fin - 1] == 0) fin--;
            var texto = System.Text.Encoding.Latin1.GetString(bytes, 0, fin);
            var linea = new System.Text.RegularExpressions.Regex(@"^(\d+)\s*=\s*""([^""]*)""", System.Text.RegularExpressions.RegexOptions.Multiline);
            var nombres = linea.Matches(texto).Select(m => m.Groups[2].Value).ToHashSet();
            int max = linea.Matches(texto).Max(m => int.Parse(m.Groups[1].Value));
            var nuevos = linea.Matches(File.ReadAllText(nr, System.Text.Encoding.Latin1)).Select(m => m.Groups[2].Value).Where(x => !nombres.Contains(x)).Distinct().ToList();
            var sb = new System.Text.StringBuilder(texto);
            foreach (var x in nuevos) sb.Append($"{++max,-5}= \"{x}\"\n");
            var salida = System.Text.RegularExpressions.Regex.Replace(sb.ToString(), @"^Num\s*=\s*\d+", $"Num  = {max}", System.Text.RegularExpressions.RegexOptions.Multiline);
            var b = System.Text.Encoding.Latin1.GetBytes(salida);
            var relleno = new byte[(16 - b.Length % 16) % 16];
            Directory.CreateDirectory(Path.Combine(root, "mod", "action"));
            File.WriteAllBytes(Path.Combine(root, "mod", "action", n + ".txt"), b.Concat(relleno).ToArray());
            notas.Add($"{n} +{nuevos.Count}");
        }
        Console.WriteLine($"Nightreign: tablas de comportamiento: {string.Join(", ", notas)}");
    }

    /// Métodos de la IA que Nightreign tiene y Elden Ring no, y por cuál se cambian (uno que no haga nada
    /// malo con los mismos argumentos). StartAttackPassedTimer pone en marcha la cuenta de un ataque que
    /// ER ya lleva sola (GetAttackPassedTime); AddObserveRegion vigila una zona del terreno de Nightreign.
    static readonly (string de, string a)[] METODOS_NR =
    {
        ("StartAttackPassedTimer", "GetAttackPassedTime"),
        ("AddObserveRegion", "IsInsideObserve"),
        // Objetivo que en Nightreign marca el evento del mapa: aquí nadie lo marca y el acto no hace nada.
        // Heolstor invoca sus armas contra él (3023-3030): sin arma, su segunda fase no lanzaba nada
        // (18-10). Contra el jugador sí las invoca.
        ("TARGET_EVENT", "TARGET_ENE_0"),
    };

    static Dictionary<string, double>? numerosEr, numerosNr;

    /// Globales numéricas (goal_list, ai_define...) de los scripts comunes de un juego.
    static Dictionary<string, double> NumerosComunes(string bnd)
    {
        var d = new Dictionary<string, double>();
        if (!File.Exists(bnd)) return d;
        foreach (var f in BND4.Read(bnd).Files.Where(f => f.Name.EndsWith(".lua")))
            try { foreach (var (k, v) in Lua50.Leer(f.Bytes.ToArray()).Numeros()) d[k] = v; } catch { }
        return d;
    }

    /// La IA de un jefe de Nightreign para Elden Ring: al principio, las constantes que usa y que en
    /// Nightreign definían sus scripts comunes (su GOAL_..._Battle, TARGET_ENE_1...), y sus métodos de
    /// Nightreign cambiados por los de METODOS_NR.
    static byte[] IaParaER(string root, byte[] bytes, List<string> notas)
    {
        numerosEr ??= NumerosComunes(Path.Combine(root, "extract", "script", "aicommon.luabnd.dcx"));
        if (numerosNr == null)
        {
            numerosNr = NumerosComunes(Path.Combine(root, "extract_nr", "script", "aicommon.luabnd.dcx"));
            // Los objetivos de los jefes del DLC de Nightreign (GOAL_Artorius772000_Battle...) están en su
            // propio banco: sin ellos la IA de Artorias no arrancaba y se quedaba quieto (18-10).
            foreach (var (k, v) in NumerosComunes(Path.Combine(root, "extract_nr", "script", "aicommon_dlc01.luabnd.dcx")))
                numerosNr.TryAdd(k, v);
        }
        if (numerosEr.Count == 0)
        {
            notas.Add("sin extract/script/aicommon.luabnd.dcx: IA sin adaptar");
            return bytes;
        }
        var l = Lua50.Leer(bytes);
        var cadenas = l.Cadenas();
        var faltan = cadenas.Where(n => numerosNr.ContainsKey(n) && !numerosEr.ContainsKey(n)).ToDictionary(n => n, n => numerosNr[n]);
        l.Definir(faltan);
        if (faltan.Count > 0) notas.Add($"define {string.Join(", ", faltan.Keys.Order())}");
        foreach (var (de, a) in METODOS_NR)
        {
            int n = l.Cambiar(de, a);
            if (n > 0) notas.Add($"{de} -> {a}");
        }
        return l.Escribir();
    }

    /// Los MATBIN de los jefes en una copia del allmaterial de ER.
    static void Materiales(string root)
    {
        var erPath = Path.Combine(root, "extract", "material", "allmaterial.matbinbnd.dcx");
        var nrPath = Path.Combine(root, "extract_nr", "material", "allmaterial.matbinbnd.dcx");
        if (materialesUsados.Count == 0) return;
        if (!File.Exists(erPath) || !File.Exists(nrPath))
        {
            Console.WriteLine("Nightreign: faltan los allmaterial (Extractor <Game> extract material/allmaterial.matbinbnd.dcx, y lo mismo de Nightreign a extract_nr)");
            return;
        }
        var er = BND4.Read(erPath);
        var nr = BND4.Read(nrPath);
        // Los del DLC de Nightreign (Artorias...) van en otro banco, allmaterial_dlc01, que el diccionario
        // del Extractor no tenía: sin él, sus modelos salían grises (18-10). Se dejan en la carpeta
        // normal de personajes de ER, como los demás.
        var nrDlc = Path.Combine(root, "extract_nr", "material", "allmaterial_dlc01.matbinbnd.dcx");
        if (File.Exists(nrDlc))
            foreach (var f in BND4.Read(nrDlc).Files)
            {
                var nombre = Path.GetFileName(f.Name);
                if (nr.Files.Any(x => Path.GetFileName(x.Name).Equals(nombre, StringComparison.OrdinalIgnoreCase))) continue;
                f.Name = RUTA_NR + @"\material\matbin\Chr\matxml\" + nombre;
                nr.Files.Add(f);
            }
        else
            Console.WriteLine("Nightreign: falta extract_nr/material/allmaterial_dlc01.matbinbnd.dcx (materiales del DLC: Artorias...)");
        // Shaders de ER por nombre de archivo (en ER la ruta es entera; en NR, solo el nombre).
        var shaders = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        var porNombre = new Dictionary<string, MATBIN>(StringComparer.OrdinalIgnoreCase);
        var porShader = new Dictionary<string, MATBIN>(StringComparer.OrdinalIgnoreCase);
        foreach (var f in er.Files)
        {
            try
            {
                var m = MATBIN.Read(f.Bytes);
                shaders.TryAdd(Path.GetFileName(m.ShaderPath), m.ShaderPath);
                porShader.TryAdd(Path.GetFileName(m.ShaderPath), m);
                porNombre.TryAdd(Path.GetFileName(f.Name), m);
            }
            catch { }
        }
        var nombres = er.Files.Select(f => Path.GetFileName(f.Name)).ToHashSet(StringComparer.OrdinalIgnoreCase);
        var principal = Principales(er, nr);
        var aMano = new Dictionary<string, JsonElement>(StringComparer.OrdinalIgnoreCase);
        var json = Path.Combine(root, "data", "nightreign.json");
        if (File.Exists(json) && JsonDocument.Parse(File.ReadAllText(json)).RootElement.TryGetProperty("materiales", out var mats))
            foreach (var x in mats.EnumerateObject()) aMano[x.Name + ".matbin"] = x.Value.Clone();
        int id = er.Files.Max(f => f.ID) + 1, n = 0;
        var cambiados = new List<string>();
        foreach (var f in nr.Files.Where(f => materialesUsados.Contains(Path.GetFileName(f.Name))))
        {
            var nombre = Path.GetFileName(f.Name);
            if (nombres.Contains(nombre)) continue;
            var m = MATBIN.Read(f.Bytes);
            var spx = Path.GetFileName(m.ShaderPath);
            if (shaders.TryGetValue(spx, out var ruta))
                m.ShaderPath = ruta;
            else
            {
                // Capas que el juego pone por encima con efectos (C_Ov...): sin equivalente, fuera.
                if (spx.StartsWith("C_Ov", StringComparison.OrdinalIgnoreCase))
                {
                    cambiados.Add($"{nombre}: {spx} (capa de efecto) sin pasar");
                    continue;
                }
                var sustituto = SHADER_DE_ER.FirstOrDefault(s => spx.Contains(s.spx, StringComparison.OrdinalIgnoreCase));
                MATBIN? tpl = null;
                if (aMano.TryGetValue(nombre, out var dm) && dm.TryGetProperty("plantilla", out var pl)) porNombre.TryGetValue(pl.GetString()!, out tpl);
                if (tpl == null && sustituto.plantilla != null) porNombre.TryGetValue(sustituto.plantilla, out tpl);
                if (tpl == null)
                {
                    var parecido = ShaderParecido(spx, shaders.Keys);
                    tpl = porShader[parecido];
                }
                var nuevo = ConTexturas(tpl, m, principal);
                var alb = nuevo.Samplers.Where(x => Papel(x.Type) == "albedo").OrderByDescending(x => principal.GetValueOrDefault(x.Type)).FirstOrDefault()?.Path;
                cambiados.Add($"{nombre}: {spx} -> {Path.GetFileName(nuevo.ShaderPath)} (albedo {(string.IsNullOrEmpty(alb) ? "NINGUNO" : alb)})");
                m = nuevo;
            }
            if (aMano.TryGetValue(nombre, out var datos))
            {
                TexturasAMano(m, datos, principal);
                cambiados.Add($"{nombre}: texturas a mano");
            }
            er.Files.Add(new BinderFile(f.Flags, id++, f.Name.Replace(RUTA_NR, RUTA_ER), m.Write()));
            n++;
        }
        var salida = Path.Combine(root, "mod", "material", "allmaterial.matbinbnd.dcx");
        Directory.CreateDirectory(Path.GetDirectoryName(salida)!);
        er.Write(salida, er.Compression);
        Console.WriteLine($"Nightreign: {n} materiales en allmaterial");
        foreach (var x in cambiados) Console.WriteLine($"   shader de ER en {x}");
    }

    /// Efectos de los jefes en una copia del banco común grande de ER (sfxbnd_commoneffects, 88 MB), con
    /// la numeración de ER:
    /// efectos desde 0, texturas 100000, modelos 200000, animaciones 300000 y listas de recursos 400000
    /// (la de cada efecto, con su mismo número). Las texturas y modelos de partículas que piden sus
    /// listas y no están en ningún común de ER salen del común de Nightreign. Así entró la onda del
    /// Golpetón en septiembre. En el común del DLC 2 (16-10, más pequeño) el juego carga el banco pero
    /// no dibuja lo añadido: el Fundidor sin fuego y el Rey sin nombre sin rayos (17-10); en el
    /// banco, su FXR no salía ni en una fila de efecto visual de ER que sí se ve.
    static void EfectosEnComun(string root, List<BinderFile> efectos)
    {
        var dlc = Path.Combine(root, "extract", "sfx", "sfxbnd_commoneffects.ffxbnd.dcx");
        var salida = Path.Combine(root, "mod", "sfx", "sfxbnd_commoneffects.ffxbnd.dcx");
        // La copia del común del DLC 2 de antes, fuera.
        var viejoDlc = Path.Combine(root, "mod", "sfx", "sfxbnd_commoneffects_dlc02.ffxbnd.dcx");
        if (File.Exists(viejoDlc)) File.Delete(viejoDlc);
        if (efectos.Count == 0)
        {
            if (File.Exists(salida)) File.Delete(salida);
            return;
        }
        if (!File.Exists(dlc))
        {
            Console.WriteLine("Nightreign: falta extract/sfx/sfxbnd_commoneffects.ffxbnd.dcx (Extractor <Game de ER> extract sfx/sfxbnd_commoneffects): jefes sin efectos");
            return;
        }
        var bnd = BND4.Read(dlc);
        // Lo que ya tiene ER en sus comunes, por nombre sin extensión.
        var deEr = new HashSet<string>(bnd.Files.Select(f => Path.GetFileNameWithoutExtension(f.Name)), StringComparer.OrdinalIgnoreCase);
        var delDlc = Path.Combine(root, "extract", "sfx", "sfxbnd_commoneffects_dlc02.ffxbnd.dcx");
        if (File.Exists(delDlc))
            foreach (var f in BND4.Read(delDlc).Files) deEr.Add(Path.GetFileNameWithoutExtension(f.Name));
        int Siguiente(int desde) => Math.Max(desde, bnd.Files.Where(f => f.ID >= desde && f.ID < desde + 100_000).Select(f => f.ID + 1).DefaultIfEmpty(desde).Max());
        string Carpeta(string ext) => ext switch { ".fxr" => "effect", ".tpf" => "tex", ".flver" => "model", ".anibnd" => "hkx", _ => "ResourceList" };
        var flags = bnd.Files[0].Flags;
        int nFx = 0, nRec = 0, chocan = 0;
        var pedidos = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        // Efectos de un jefe con el mismo número que uno de ER: se renumeran (690000000 + número) y se
        // corrige el número en los efectos que los crean como hijos. Si no, salía el de ER: el 632515 del
        // Rey sin nombre (hijo de su 679010) es en ER una cabeza de dragón, y le salía en la lanza (18-10).
        var renumerar = efectos.Where(f => f.Name.EndsWith(".fxr") && deEr.Contains(Path.GetFileNameWithoutExtension(f.Name)))
            .Select(f => int.Parse(Path.GetFileNameWithoutExtension(f.Name)[1..])).Distinct().ToDictionary(x => x, x => 690_000_000 + x % 10_000_000);
        if (renumerar.Count > 0)
        {
            foreach (var f in efectos.Where(f => f.Name.EndsWith(".fxr") || f.Name.EndsWith(".ffxreslist")))
            {
                var nombre = Path.GetFileNameWithoutExtension(f.Name);
                if (int.TryParse(nombre[1..], out var n) && renumerar.TryGetValue(n, out var nuevoId))
                    f.Name = f.Name.Replace(nombre, $"f{nuevoId:D9}");
                if (!f.Name.EndsWith(".fxr")) continue;
                var bytes = f.Bytes.ToArray();
                for (int i = 0; i + 4 <= bytes.Length; i += 4)
                    if (renumerar.TryGetValue(BitConverter.ToInt32(bytes, i), out var nid)) BitConverter.GetBytes(nid).CopyTo(bytes, i);
                f.Bytes = bytes;
            }
            Console.WriteLine($"Nightreign: efectos renumerados por chocar con ER: {string.Join(", ", renumerar.Select(x => $"{x.Key} -> {x.Value}"))}");
        }
        // Efectos que no se pasan («efectos_fuera» en data/nightreign.json): el 632515 del Rey sin nombre,
        // que el conversor no lee y en ER se quedaba enganchado tras la onda de su lanza, con mucho lag al
        // mirarlo (18-10). Quien lo crea como hijo apunta a un número que no existe y no sale nada.
        var fuera = new HashSet<int>();
        var json = Path.Combine(root, "data", "nightreign.json");
        if (File.Exists(json) && JsonDocument.Parse(File.ReadAllText(json)).RootElement.TryGetProperty("efectos_fuera", out var ef))
            foreach (var x in ef.EnumerateArray()) { fuera.Add(x.GetInt32()); if (renumerar.TryGetValue(x.GetInt32(), out var r2)) fuera.Add(r2); }
        foreach (var f in efectos.Where(f => f.Name.EndsWith(".fxr")))
        {
            var nombre = Path.GetFileNameWithoutExtension(f.Name);
            if (int.TryParse(nombre[1..], out var idFx) && fuera.Contains(idFx)) continue;
            if (deEr.Contains(nombre)) { chocan++; continue; }
            int id = Siguiente(0);
            bnd.Files.Add(new BinderFile(flags, id, $@"{RUTA_ER}\sfx\effect\{nombre}.fxr", f.Bytes.ToArray()));
            deEr.Add(nombre);
            nFx++;
            var lista = efectos.FirstOrDefault(x => x.Name.EndsWith(".ffxreslist") && Path.GetFileNameWithoutExtension(x.Name) == nombre);
            if (lista == null) continue;
            bnd.Files.Add(new BinderFile(flags, 400_000 + id, $@"{RUTA_ER}\sfx\ResourceList\{nombre}.ffxreslist", lista.Bytes.ToArray()));
            foreach (var l in System.Text.Encoding.ASCII.GetString(lista.Bytes.ToArray()).Split('\n', '\r'))
                if (l.Trim().Trim('\0').Length > 0) pedidos.Add(Path.GetFileNameWithoutExtension(l.Trim().Trim('\0')));
        }
        // Recursos que faltan: del banco del jefe o del común de Nightreign.
        var faltan = pedidos.Where(p => !deEr.Contains(p)).ToList();
        var nrComun = Path.Combine(root, "extract_nr", "sfx", "sfxbnd_commoneffects.ffxbnd.dcx");
        var fuentes = efectos.Where(f => !f.Name.EndsWith(".fxr") && !f.Name.EndsWith(".ffxreslist")).ToList();
        if (faltan.Count > 0 && File.Exists(nrComun)) fuentes.AddRange(BND4.Read(nrComun).Files);
        var sinFuente = new List<string>();
        foreach (var p in faltan)
        {
            var f = fuentes.FirstOrDefault(x => Path.GetFileNameWithoutExtension(x.Name).Equals(p, StringComparison.OrdinalIgnoreCase)
                && !x.Name.EndsWith(".fxr") && !x.Name.EndsWith(".ffxreslist"));
            if (f == null) { sinFuente.Add(p); continue; }
            var ext = Path.GetExtension(f.Name).ToLowerInvariant();
            int baseId = ext switch { ".tpf" => 100_000, ".flver" => 200_000, ".anibnd" => 300_000, _ => -1 };
            if (baseId < 0) { sinFuente.Add(p); continue; }
            var bytes = f.Bytes.ToArray();
            if (ext == ".flver") bytes = FlverDeNRaER(bytes);
            bnd.Files.Add(new BinderFile(flags, Siguiente(baseId), $@"{RUTA_ER}\sfx\{Carpeta(ext)}\{Path.GetFileName(f.Name.Replace(RUTA_NR, RUTA_ER))}", bytes));
            deEr.Add(p);
            nRec++;
        }
        bnd.Files.Sort((a, b) => a.ID.CompareTo(b.ID));
        Directory.CreateDirectory(Path.GetDirectoryName(salida)!);
        bnd.Write(salida, bnd.Compression);
        Console.WriteLine($"Nightreign: {nFx} efectos y {nRec} recursos al común ({chocan} con un número que ER ya usa, fuera)" +
            (sinFuente.Count > 0 ? $"; recursos sin encontrar: {string.Join(", ", sinFuente.Take(12))}" : ""));
    }

    /// FXR de NR a ER con tools/fxr (Node y @cccode/fxr, de EvenTorset): convierte de verdad entre los
    /// dos juegos. El recorte de campos en los bytes (FxrDeNRaER) dejaba lo que ER no tiene, como las
    /// curvas ComponentHermite de NR, y el juego cargaba los efectos sin dibujarlos (Fundidor sin
    /// fuego, 18-10). Sin Node o sin `npm i` en tools/fxr, y para los que la librería no lee, el recorte.
    static void ConvertirFxr(string root, List<BinderFile> fxr, Dictionary<short, (int, int)>? canon)
    {
        if (fxr.Count == 0) return;
        var tmp = Path.Combine(Path.GetTempPath(), "rogue_fxr_" + Environment.ProcessId);
        var dentro = Path.Combine(tmp, "nr");
        var fuera = Path.Combine(tmp, "er");
        Directory.CreateDirectory(dentro);
        foreach (var f in fxr) File.WriteAllBytes(Path.Combine(dentro, Path.GetFileName(f.Name)), f.Bytes.ToArray());
        var dir = Path.Combine(root, "tools", "fxr");
        string informe = "";
        if (Directory.Exists(Path.Combine(dir, "node_modules", "@cccode", "fxr")))
        {
            try
            {
                var psi = new System.Diagnostics.ProcessStartInfo("node", $"convertir.mjs \"{dentro}\" \"{fuera}\"")
                    { WorkingDirectory = dir, RedirectStandardOutput = true, UseShellExecute = false };
                using var pr = System.Diagnostics.Process.Start(psi)!;
                informe = pr.StandardOutput.ReadToEnd().Trim();
                pr.WaitForExit();
            }
            catch (Exception e) { informe = "sin Node: " + e.Message; }
        }
        else informe = "falta tools/fxr/node_modules (npm i en tools/fxr)";
        int libreria = 0, recorte = 0;
        foreach (var f in fxr)
        {
            var hecho = Path.Combine(fuera, Path.GetFileName(f.Name));
            if (File.Exists(hecho)) { f.Bytes = File.ReadAllBytes(hecho); libreria++; continue; }
            var bytes = f.Bytes.ToArray();
            if (canon != null && bytes.Length > 0x40 && bytes[0] == (byte)'F' && bytes[1] == (byte)'X' && bytes[2] == (byte)'R') FxrDeNRaER(bytes, canon);
            f.Bytes = bytes;
            recorte++;
        }
        try { Directory.Delete(tmp, true); } catch { }
        Console.WriteLine($"Nightreign: FXR convertidos con @cccode/fxr {libreria}, con el recorte {recorte} ({informe.Split((char)10).LastOrDefault()})");
    }

    /// Campos que usa ER en cada tipo de acción de FXR (el más común en sus efectos comunes).
    static Dictionary<short, (int, int)>? CamposFxrDeER(string root)
    {
        var erBnd = Path.Combine(root, "extract", "sfx", "sfxbnd_commoneffects.ffxbnd.dcx");
        if (!File.Exists(erBnd)) return null;
        static IEnumerable<FXR3.Action> Acciones(FXR3.Container c)
        {
            foreach (var a in c.Actions) yield return a;
            foreach (var e in c.Effects) foreach (var a in e.Actions) yield return a;
            foreach (var s in c.Containers) foreach (var a in Acciones(s)) yield return a;
        }
        var cuentas = new Dictionary<short, Dictionary<(int, int), int>>();
        foreach (var f in BND4.Read(erBnd).Files.Where(f => f.Name.EndsWith(".fxr")))
        {
            FXR3 fx;
            try { var b = f.Bytes.ToArray(); b[4] = 0; fx = FXR3.Read(b); } catch { continue; }
            foreach (var a in Acciones(fx.RootContainer))
            {
                if (!cuentas.TryGetValue(a.Id, out var d)) cuentas[a.Id] = d = new();
                var k = (a.Fields1.Count, a.Fields2.Count);
                d[k] = d.GetValueOrDefault(k) + 1;
            }
        }
        return cuentas.ToDictionary(x => x.Key, x => x.Value.OrderByDescending(y => y.Value).First().Key);
    }

    /// FXR de NR a ER en los bytes (el escritor de FXR3 estropea hasta los efectos de ER): cada acción
    /// (registros de 64 bytes) apunta a su bloque de campos, Fields1 y luego Fields2. Se dejan al
    /// principio del bloque los que usa ER y se bajan los contadores; los sobrantes quedan sin referencia.
    static (int recortados, int raras) FxrDeNRaER(byte[] bytes, Dictionary<short, (int, int)> canon)
    {
        int cut = 0, unknown = 0;
        bytes[4] = 0;
        int actOff = BitConverter.ToInt32(bytes, 0x38), actCount = BitConverter.ToInt32(bytes, 0x3C);
        for (int k = 0; k < actCount; k++)
        {
            int r = actOff + k * 64;
            short aid = BitConverter.ToInt16(bytes, r);
            int fc1 = BitConverter.ToInt32(bytes, r + 8), fc2 = BitConverter.ToInt32(bytes, r + 20), fo = BitConverter.ToInt32(bytes, r + 32);
            if (!canon.TryGetValue(aid, out var c)) { unknown++; continue; }
            int n1 = Math.Min(fc1, c.Item1), n2 = Math.Min(fc2, c.Item2);
            if (n1 == fc1 && n2 == fc2) continue;
            var f2 = bytes.AsSpan(fo + fc1 * 4, n2 * 4).ToArray();
            f2.CopyTo(bytes, fo + n1 * 4);
            BitConverter.GetBytes(n1).CopyTo(bytes, r + 8);
            BitConverter.GetBytes(n2).CopyTo(bytes, r + 20);
            cut += fc1 - n1 + fc2 - n2;
        }
        return (cut, unknown);
    }

    // --- Parámetros ---

    static readonly Dictionary<string, Dictionary<int, Dictionary<string, string>>> csvs = new();

    /// Filas de un parámetro de Nightreign (extract_nr/params/<nombre>.csv): id -> campo -> valor.
    static Dictionary<int, Dictionary<string, string>> Csv(string root, string nombre)
    {
        if (csvs.TryGetValue(nombre, out var hecho)) return hecho;
        var filas = new Dictionary<int, Dictionary<string, string>>();
        using var r = new StreamReader(Path.Combine(root, "extract_nr", "params", nombre + ".csv"));
        var cab = r.ReadLine()!.Split(',');
        string? l;
        while ((l = r.ReadLine()) != null)
        {
            var v = l.Split(',');
            var d = new Dictionary<string, string>(cab.Length);
            for (int i = 0; i < cab.Length && i < v.Length; i++) d[cab[i]] = v[i];
            filas[int.Parse(v[0])] = d;
        }
        return csvs[nombre] = filas;
    }

    static double Num(string s) => double.Parse(s, CultureInfo.InvariantCulture);

    /// Pone en la fila de ER los valores de la de NR con el mismo nombre de campo (menos `salvo`).
    static int Rellenar(Param.Row fila, Dictionary<string, string> nr, ICollection<string>? salvo = null)
    {
        int puestos = 0;
        foreach (var col in fila.Columns)
        {
            var campo = col.Def.InternalName;
            if (salvo != null && salvo.Contains(campo)) continue;
            if (!nr.TryGetValue(campo, out var s) || col.Def.DisplayType is PARAMDEF.DefType.dummy8 or PARAMDEF.DefType.fixstr or PARAMDEF.DefType.fixstrW)
                continue;
            try
            {
                object v = col.ValueType == typeof(float) ? float.Parse(s, CultureInfo.InvariantCulture)
                    : col.ValueType == typeof(double) ? Num(s)
                    : Convert.ChangeType(Num(s), col.ValueType, CultureInfo.InvariantCulture);
                col.SetValue(fila, v);
                puestos++;
            }
            catch (OverflowException) { }
        }
        return puestos;
    }

    /// Fila nueva con los valores por defecto de ER (los campos que NR no tiene se quedan así).
    static Param.Row Nueva(Param p, int id, string nombre)
    {
        var fila = new Param.Row(id, nombre, p);
        foreach (var col in fila.Columns)
        {
            if (col.Def.Default == null || col.Def.DisplayType is PARAMDEF.DefType.dummy8 or PARAMDEF.DefType.fixstr or PARAMDEF.DefType.fixstrW) continue;
            try { col.SetValue(fila, Convert.ChangeType(col.Def.Default, col.ValueType, CultureInfo.InvariantCulture)); } catch { }
        }
        return fila;
    }

    static void Meter(Param p, Param.Row fila)
    {
        int i = p.Rows.TakeWhile(x => x.ID < fila.ID).Count();
        if (i < p.Rows.Count && p.Rows[i].ID == fila.ID) p.RemoveRowAt(i);
        p.InsertRow(i, fila);
    }

    /// Filas de NR que necesita cada jefe, y sus referencias a efectos (SpEffect) y a sus efectos
    /// visuales. Lo que ya tiene ER (efectos comunes) se queda como en ER.
    public static void Parametros(string root, BND4 reg, Func<BinderFile, Param> cargar, List<Jefe> jefes, List<Apariencia> apariencias)
    {
        jefes = jefes.Where(j => File.Exists(Path.Combine(root, "extract_nr", "chr", Modelo(j) + ".chrbnd.dcx"))).ToList();
        apariencias = apariencias.Where(a => File.Exists(Path.Combine(root, "mod", "parts", a.Parte + ".partsbnd.dcx"))).ToList();
        if (jefes.Count == 0 && apariencias.Count == 0) return;
        var archivos = new Dictionary<string, BinderFile>();
        var ps = new Dictionary<string, Param>();
        Param P(string n)
        {
            if (!ps.TryGetValue(n, out var p))
            {
                archivos[n] = reg.Files.First(x => x.Name.EndsWith("\\" + n + ".param"));
                ps[n] = p = cargar(archivos[n]);
            }
            return p;
        }
        var spNr = Csv(root, "SpEffectParam");
        var vfxNr = Csv(root, "SpEffectVfxParam");
        var atkNr = Csv(root, "AtkParam_Npc");
        var bulNr = Csv(root, "Bullet");
        var behNr = Csv(root, "BehaviorParam");
        foreach (var j in jefes)
        {
            var c = Modelo(j);
            var atk = new SortedSet<int>();
            var bul = new SortedSet<int>();
            var sp = new SortedSet<int>();     // efectos referenciados (de ER o de NR)
            var beh = new SortedSet<int>();
            // Comportamientos de su variación (2_<variación>_<juicio>) y ataques y proyectiles de su rango.
            foreach (var npc in j.Npc)
            {
                var variacion = (int)Num(Csv(root, "NpcParam")[npc]["behaviorVariationId"]);
                foreach (var (id, b) in behNr.Where(x => (x.Key - 200_000_000) / 1000 == variacion))
                {
                    beh.Add(id);
                    var refId = (int)Num(b["refId"]);
                    switch ((int)Num(b["refType"]))
                    {
                        case 0: atk.Add(refId); break;
                        case 1: bul.Add(refId); break;
                        case 2: sp.Add(refId); break;
                    }
                }
                foreach (var k in Enumerable.Range(0, 32))
                    if (Csv(root, "NpcParam")[npc].TryGetValue($"spEffectID{k}", out var s)) sp.Add((int)Num(s));
                if (j.EfectosNpc?.GetValueOrDefault(npc) is { } extra) foreach (var x in extra) sp.Add(x);
            }
            foreach (var id in atkNr.Keys.Where(x => x / 1000 == j.Chr)) atk.Add(id);
            foreach (var id in bulNr.Keys.Where(x => x / 1000 == j.Chr)) bul.Add(id);
            // Proyectiles que sueltan otros proyectiles, y sus ataques y efectos.
            var pend = new Queue<int>(bul);
            while (pend.Count > 0)
            {
                if (!bulNr.TryGetValue(pend.Dequeue(), out var b)) continue;
                atk.Add((int)Num(b["atkId_Bullet"]));
                foreach (var campo in new[] { "HitBulletID", "intervalCreateBulletId" })
                    if (b.TryGetValue(campo, out var s) && Num(s) > 0 && bul.Add((int)Num(s))) pend.Enqueue((int)Num(s));
                foreach (var campo in new[] { "spEffectIDForShooter", "spEffectId0", "spEffectId1", "spEffectId2", "spEffectId3", "spEffectId4" })
                    if (b.TryGetValue(campo, out var s)) sp.Add((int)Num(s));
            }
            foreach (var id in atk)
                if (atkNr.TryGetValue(id, out var a))
                    for (int k = 0; k < 5; k++) sp.Add((int)Num(a[$"spEffectId{k}"]));
            // Efectos que ponen sus animaciones (eventos 66 y 67 del TAE).
            var ani = BND4.Read(Path.Combine(root, "extract_nr", "chr", c + ".anibnd.dcx"));
            foreach (var tf in ani.Files.Where(f => f.Name.EndsWith(".tae")))
                foreach (var an in TAE.Read(tf.Bytes).Animations)
                    foreach (var ev in an.Events.Where(e => e.Type is 66 or 67))
                        sp.Add(BitConverter.ToInt32(ev.GetParameterBytes(false), 0));

            // Efectos: los que ER no tiene salen de NR, con los que a su vez encadenan.
            var spEr = P("SpEffectParam");
            var vfxEr = P("SpEffectVfxParam");
            var spPasar = new SortedSet<int>();
            var vfxPasar = new SortedSet<int>();
            var sinFila = new SortedSet<int>();
            var cola = new Queue<int>(sp.Where(x => x > 0));
            while (cola.Count > 0)
            {
                var id = cola.Dequeue();
                if (spEr[id] != null || spPasar.Contains(id)) continue;
                if (!spNr.TryGetValue(id, out var s)) { sinFila.Add(id); continue; }
                spPasar.Add(id);
                foreach (var campo in new[] { "replaceSpEffectId", "cycleOccurrenceSpEffectId", "atkOccurrenceSpEffectId", "accumuOverFireId", "accumuUnderFireId", "applyIdOnGetSoul", "spiritDeathSpEffectId" })
                    if (s.TryGetValue(campo, out var v) && Num(v) > 0) cola.Enqueue((int)Num(v));
                foreach (var campo in new[] { "vfxId", "vfxId1", "vfxId2", "vfxId3", "vfxId4", "vfxId5", "vfxId6", "vfxId7" })
                    if (s.TryGetValue(campo, out var v) && Num(v) > 0 && vfxEr[(int)Num(v)] == null && vfxNr.ContainsKey((int)Num(v))) vfxPasar.Add((int)Num(v));
            }

            // Filas nuevas. Las comunes que ya tiene ER (un ataque 1, por ejemplo) se quedan las de ER; las
            // del rango del jefe sí se pasan aunque choquen (y se avisa).
            int Pasar(string param, IEnumerable<int> ids, Dictionary<int, Dictionary<string, string>> nr)
            {
                var p = P(param);
                int n = 0;
                foreach (var id in ids.Where(x => x > 0 && nr.ContainsKey(x)))
                {
                    bool suyo = param is "BehaviorParam" or "NpcThinkParam" || id / 1000 == j.Chr;
                    if (p[id] != null)
                    {
                        if (!suyo) continue;
                        Console.WriteLine($"Nightreign {c}: {param} {id} ya existe en ER; se sobrescribe");
                    }
                    var fila = Nueva(p, id, $"RogueRing NR {c}");
                    Rellenar(fila, nr[id]);
                    Meter(p, fila);
                    n++;
                }
                return n;
            }
            int nBeh = Pasar("BehaviorParam", beh, behNr);
            int nAtk = Pasar("AtkParam_Npc", atk, atkNr);
            int nBul = Pasar("Bullet", bul, bulNr);
            int nSp = Pasar("SpEffectParam", spPasar, spNr);
            int nVfx = Pasar("SpEffectVfxParam", vfxPasar, vfxNr);
            int nThink = Pasar("NpcThinkParam", j.Think, Csv(root, "NpcThinkParam"));
            // Críticos del jugador sobre el jefe (ThrowParam con DefChrId = su modelo: espaldas y estocada
            // tras romperle la postura, con el mismo esquema de tipos y animaciones que ER). Sin ellos no se
            // le puede hacer crítico. Sus agarres al jugador no: sus animaciones de víctima (70660...) son en
            // ER las de agarres de otros enemigos.
            var throwNr = Csv(root, "ThrowParam");
            int nThrow = Pasar("ThrowParam", throwNr.Where(x => (int)Num(x.Value["DefChrId"]) == j.Chr).Select(x => x.Key).ToList(), throwNr);

            // NpcParam: la plantilla de ER con lo de NR encima, salvo equipo, tipo, recompensas y
            // correcciones que en NR son otras tablas. Efectos que no están en ninguno de los dos: fuera.
            var npcEr = P("NpcParam");
            var tpl = npcEr[j.Plantilla] ?? throw new Exception($"Nightreign {c}: no existe la plantilla NpcParam {j.Plantilla}");
            string[] deEr = { "teamType", "npcType", "moveType", "multiPlayCorrectionParamId", "isSoulGetByBoss", "getSoul",
                "itemLotId_enemy", "itemLotId_map", "nameId", "GameClearSpEffectID", "dlcGameClearSpEffectID", "chrActivateConditionParamId" };
            var lockEr = P("LockCamParam");
            var lockNr = Csv(root, "LockCamParam");
            foreach (var npc in j.Npc)
            {
                var nr = Csv(root, "NpcParam")[npc];
                var fila = new Param.Row(tpl, npcEr) { ID = npc, Name = $"RogueRing NR {j.Nombre}" };
                Rellenar(fila, nr, deEr);
                for (int k = 0; k < 32; k++)
                {
                    var cel = fila[$"spEffectID{k}"];
                    if (cel == null) continue;
                    var v = Convert.ToInt32(cel.Value.Value);
                    if (v > 0 && spEr[v] == null) cel.Value.SetValue(-1);
                }
                foreach (var extra in j.EfectosNpc?.GetValueOrDefault(npc) ?? [])
                {
                    var hueco = Enumerable.Range(0, 32).Select(k => fila[$"spEffectID{k}"]).FirstOrDefault(cel => cel != null && Convert.ToInt32(cel.Value.Value) <= 0);
                    if (hueco == null) { Console.WriteLine($"Nightreign {c}: NpcParam {npc} sin hueco para el efecto {extra}"); continue; }
                    hueco.Value.SetValue(extra);
                }
                if (j.Vida is int vida) fila["hp"]!.Value.SetValue((uint)vida);
                // Resistencias que ER no tiene: las de la plantilla.
                foreach (var col in fila.Columns.Where(x => x.Def.InternalName.StartsWith("resistCorrectId_")))
                {
                    var v = Convert.ToInt32(col.GetValue(fila));
                    if (v > 0 && P("ResistCorrectParam")[v] == null) col.SetValue(fila, col.GetValue(tpl));
                }
                // Cámara al fijar: la suya.
                var cam = Convert.ToInt32(fila["lockCameraParamId"]!.Value.Value);
                if (cam > 0 && lockEr[cam] == null && lockNr.ContainsKey(cam))
                {
                    var lc = Nueva(lockEr, cam, $"RogueRing NR {c}");
                    Rellenar(lc, lockNr[cam]);
                    Meter(lockEr, lc);
                }
                Meter(npcEr, fila);
            }
            // Memoria del modelo.
            var cm = P("ChrModelParam");
            if (cm[j.Chr] == null && Csv(root, "ChrModelParam").TryGetValue(j.Chr, out var cmNr))
            {
                var fila = Nueva(cm, j.Chr, $"RogueRing NR {c}");
                Rellenar(fila, cmNr);
                Meter(cm, fila);
            }
            Console.WriteLine($"Nightreign {c}: {j.Npc.Length} NpcParam, {nThink} NpcThinkParam, {nThrow} críticos, {nBeh} comportamientos, {nAtk} ataques, " +
                $"{nBul} proyectiles, {nSp} efectos y {nVfx} efectos visuales de NR ({sp.Count(x => x > 0 && spEr[x] != null)} efectos ya en ER)" +
                (sinFila.Count > 0 ? $"; sin fila en ninguno: {string.Join(", ", sinFila)}" : ""));
        }
        // Apariencias: una pieza de torso con el modelo del héroe, que oculta todo el cuerpo del jugador
        // (todas las marcas) y lleva las defensas de las cuatro piezas de ropa del personaje juntas: las
        // reducciones de daño se multiplican, resistencias, peso y aguante se suman.
        foreach (var a in apariencias)
        {
            var pr = P("EquipParamProtector");
            var piezas = a.Ropa.Where(x => x > 0).Select(x => pr[x] ?? throw new Exception($"apariencia {a.Nombre}: no existe la ropa {x}")).ToList();
            var fila = new Param.Row(pr[a.Ropa[1]]!, pr) { ID = a.Id, Name = $"RogueRing apariencia {a.Nombre}" };
            foreach (var col in fila.Columns)
            {
                var n = col.Def.InternalName;
                if (n.StartsWith("invisibleFlag")) col.SetValue(fila, Convert.ChangeType(1, col.ValueType));
                else if (n.EndsWith("DamageCutRate") && col.ValueType == typeof(float))
                    col.SetValue(fila, piezas.Aggregate(1f, (acc, x) => acc * (float)col.GetValue(x)));
                else if ((n.StartsWith("resist") || n == "weight" || n == "toughnessCorrectRate") && col.ValueType != typeof(byte[]))
                    col.SetValue(fila, Convert.ChangeType(piezas.Sum(x => Convert.ToDouble(col.GetValue(x))), col.ValueType));
            }
            fila["equipModelId"]!.Value.SetValue(Convert.ChangeType(int.Parse(a.Parte[^4..]), fila["equipModelId"]!.Value.Value.GetType()));
            Meter(pr, fila);
            Console.WriteLine($"Nightreign: apariencia {a.Nombre} en EquipParamProtector {a.Id} (modelo {a.Parte}, defensas de {string.Join("+", a.Ropa)})");
        }
        foreach (var (n, p) in ps) archivos[n].Bytes = p.Write();
    }
}
