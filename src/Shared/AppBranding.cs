using System.Reflection;

namespace SupabaseLocal.Setup;

internal static class AppBranding
{
    public static Icon WindowIcon()
    {
        using var stream = Resource("SupabaseLocal.Branding.Icon");
        using var icon = new Icon(stream);
        return (Icon)icon.Clone();
    }

    public static Control Header()
    {
        using var stream = Resource("SupabaseLocal.Branding.Logo");
        using var image = Image.FromStream(stream);
        var logo = new PictureBox { Image = new Bitmap(image), SizeMode = PictureBoxSizeMode.Zoom, Width = 48, Dock = DockStyle.Left, Margin = Padding.Empty };
        var heading = new Label { Text = "Supabase Local", Font = new Font("Segoe UI", 16, FontStyle.Bold), AutoSize = false, Dock = DockStyle.Fill, Padding = new Padding(16, 0, 0, 0), TextAlign = ContentAlignment.MiddleLeft };
        var note = new Label { Text = "Unofficial native Windows port", Dock = DockStyle.Bottom, Height = 32, ForeColor = SystemColors.GrayText, Padding = new Padding(64, 0, 0, 0) };
        var panel = new Panel { Dock = DockStyle.Top, Height = 120, Padding = new Padding(24, 16, 24, 8) };
        panel.Controls.Add(heading); panel.Controls.Add(logo); panel.Controls.Add(note);
        panel.Disposed += (_, _) => logo.Image?.Dispose();
        return panel;
    }

    private static Stream Resource(string name) => Assembly.GetExecutingAssembly().GetManifestResourceStream(name) ?? throw new InvalidOperationException("Missing Supabase brand asset: " + name);
}
