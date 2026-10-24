// Lua 5.0 compilado («LuaP»), el de la IA de Elden Ring y de Nightreign: leer, tocar y volver a escribir
// tal cual (formato de ldump.c de Lua 5.0.2). Lo usa Nightreign.cs para que la IA de un jefe de
// Nightreign encuentre en Elden Ring lo que allí venía de los scripts comunes.
sealed class Lua50
{
    public sealed class Proto
    {
        public byte[]? Source;            // con el '\0' final; null = sin nombre
        public int Line;
        public byte Nups, NumParams, IsVararg, MaxStack;
        public int[] Lines = Array.Empty<int>();
        public List<(byte[]? name, int start, int end)> Locals = new();
        public List<byte[]?> Upvalues = new();
        public List<object?> K = new();   // double, string o null
        public List<Proto> P = new();
        public List<uint> Code = new();
    }

    byte[] cabecera = Array.Empty<byte>();
    public Proto Main = new();

    const int SIZE_T = 8;
    const uint OP_LOADK = 1, OP_SETGLOBAL = 7;

    public static Lua50 Leer(byte[] b)
    {
        if (b.Length < 22 || b[0] != 0x1B || b[4] != 0x50 || b[7] != SIZE_T)
            throw new InvalidDataException("no es Lua 5.0 compilado con size_t de 8 bytes");
        var r = new BinaryReader(new MemoryStream(b));
        var l = new Lua50 { cabecera = r.ReadBytes(22) };
        l.Main = LeerProto(r);
        if (r.BaseStream.Position != b.Length) throw new InvalidDataException("sobran bytes al final");
        return l;
    }

    static byte[]? Cadena(BinaryReader r)
    {
        var n = r.ReadInt64();
        return n == 0 ? null : r.ReadBytes((int)n);
    }

    static Proto LeerProto(BinaryReader r)
    {
        var p = new Proto { Source = Cadena(r), Line = r.ReadInt32(), Nups = r.ReadByte(), NumParams = r.ReadByte(), IsVararg = r.ReadByte(), MaxStack = r.ReadByte() };
        p.Lines = Enumerable.Range(0, r.ReadInt32()).Select(_ => r.ReadInt32()).ToArray();
        int n = r.ReadInt32();
        for (int i = 0; i < n; i++) p.Locals.Add((Cadena(r), r.ReadInt32(), r.ReadInt32()));
        n = r.ReadInt32();
        for (int i = 0; i < n; i++) p.Upvalues.Add(Cadena(r));
        n = r.ReadInt32();
        for (int i = 0; i < n; i++)
        {
            var t = r.ReadByte();
            p.K.Add(t switch
            {
                0 => null,
                3 => r.ReadDouble(),
                4 => Texto(Cadena(r)),
                _ => throw new InvalidDataException($"constante de tipo {t}"),
            });
        }
        n = r.ReadInt32();
        for (int i = 0; i < n; i++) p.P.Add(LeerProto(r));
        n = r.ReadInt32();
        for (int i = 0; i < n; i++) p.Code.Add(r.ReadUInt32());
        return p;
    }

    static string? Texto(byte[]? b) => b == null ? null : System.Text.Encoding.Latin1.GetString(b, 0, b.Length - 1);
    static byte[] Bytes(string s) => System.Text.Encoding.Latin1.GetBytes(s + "\0");

    public byte[] Escribir()
    {
        var ms = new MemoryStream();
        var w = new BinaryWriter(ms);
        w.Write(cabecera);
        EscribirProto(w, Main);
        return ms.ToArray();
    }

    static void Cadena(BinaryWriter w, byte[]? s)
    {
        w.Write((long)(s?.Length ?? 0));
        if (s != null) w.Write(s);
    }

    static void EscribirProto(BinaryWriter w, Proto p)
    {
        Cadena(w, p.Source);
        w.Write(p.Line); w.Write(p.Nups); w.Write(p.NumParams); w.Write(p.IsVararg); w.Write(p.MaxStack);
        w.Write(p.Lines.Length); foreach (var x in p.Lines) w.Write(x);
        w.Write(p.Locals.Count); foreach (var (n, s, e) in p.Locals) { Cadena(w, n); w.Write(s); w.Write(e); }
        w.Write(p.Upvalues.Count); foreach (var u in p.Upvalues) Cadena(w, u);
        w.Write(p.K.Count);
        foreach (var k in p.K)
        {
            switch (k)
            {
                case null: w.Write((byte)0); break;
                case double d: w.Write((byte)3); w.Write(d); break;
                case string s: w.Write((byte)4); Cadena(w, Bytes(s)); break;
            }
        }
        w.Write(p.P.Count); foreach (var x in p.P) EscribirProto(w, x);
        w.Write(p.Code.Count); foreach (var c in p.Code) w.Write(c);
    }

    public IEnumerable<Proto> Todos() => Recorrer(Main);
    static IEnumerable<Proto> Recorrer(Proto p) { yield return p; foreach (var h in p.P) foreach (var x in Recorrer(h)) yield return x; }

    /// Cadenas constantes de todo el script (nombres de globales y de métodos, textos).
    public HashSet<string> Cadenas() => Todos().SelectMany(p => p.K.OfType<string>()).ToHashSet();

    /// Globales que el script pone a un número con «LOADK r, num; SETGLOBAL r, nombre» (las constantes
    /// de goal_list, ai_define...).
    public Dictionary<string, double> Numeros()
    {
        var d = new Dictionary<string, double>();
        foreach (var p in Todos())
            for (int i = 0; i + 1 < p.Code.Count; i++)
            {
                uint a = p.Code[i], b = p.Code[i + 1];
                if ((a & 63) != OP_LOADK || (b & 63) != OP_SETGLOBAL || (a >> 24) != (b >> 24)) continue;
                if (p.K[(int)(a >> 6 & 0x3FFFF)] is double v && p.K[(int)(b >> 6 & 0x3FFFF)] is string n) d[n] = v;
            }
        return d;
    }

    /// Al principio del script, globales con su valor (como si estuvieran en goal_list).
    public void Definir(IDictionary<string, double> globales)
    {
        if (globales.Count == 0) return;
        var p = Main;
        // Registro libre: tras los parámetros y el «arg» de las funciones con argumentos variables.
        uint reg = (uint)(p.NumParams + (p.IsVararg != 0 ? 1 : 0));
        if (p.MaxStack < reg + 1) p.MaxStack = (byte)(reg + 1);
        var nuevas = new List<uint>();
        foreach (var (nombre, valor) in globales)
        {
            int kv = p.K.Count; p.K.Add(valor);
            int kn = p.K.Count; p.K.Add(nombre);
            nuevas.Add(OP_LOADK | (uint)kv << 6 | reg << 24);
            nuevas.Add(OP_SETGLOBAL | (uint)kn << 6 | reg << 24);
        }
        p.Code.InsertRange(0, nuevas);
        if (p.Lines.Length > 0) p.Lines = Enumerable.Repeat(p.Lines[0], nuevas.Count).Concat(p.Lines).ToArray();
    }

    /// Cambia una cadena constante (un método que Elden Ring no tiene por otro que sí).
    public int Cambiar(string de, string a)
    {
        int n = 0;
        foreach (var p in Todos())
            for (int i = 0; i < p.K.Count; i++)
                if (p.K[i] is string s && s == de) { p.K[i] = a; n++; }
        return n;
    }
}
