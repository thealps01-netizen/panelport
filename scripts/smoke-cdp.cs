// Probe-only helpers for the save-password check in CI (scripts/smoke-windows.ps1).
// Compiled into PowerShell with Add-Type; talks to WebView2's DevTools port on 127.0.0.1.
using System;
using System.Net.Http;
using System.Net.WebSockets;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading;

public static class Cdp
{
    static int nextId = 1;

    /// Returns the webSocketDebuggerUrl of the first page whose URL contains `needle`.
    public static string FindTarget(int port, string needle)
    {
        using (var http = new HttpClient())
        {
            string json = http.GetStringAsync("http://127.0.0.1:" + port + "/json/list").Result;
            foreach (Match m in Regex.Matches(json, "\\{[^{}]*\\}"))
            {
                string block = m.Value;
                var url = Regex.Match(block, "\"url\"\\s*:\\s*\"([^\"]*)\"").Groups[1].Value;
                var ws = Regex.Match(block, "\"webSocketDebuggerUrl\"\\s*:\\s*\"([^\"]*)\"").Groups[1].Value;
                if (url.Contains(needle) && ws.Length > 0) return ws;
            }
        }
        return null;
    }

    /// Sends one CDP command and returns the raw JSON reply.
    public static string Send(string wsUrl, string method, string paramsJson)
    {
        using (var ws = new ClientWebSocket())
        {
            ws.ConnectAsync(new Uri(wsUrl), CancellationToken.None).Wait();
            int id = Interlocked.Increment(ref nextId);
            string msg = "{\"id\":" + id + ",\"method\":\"" + method + "\",\"params\":" + paramsJson + "}";
            ws.SendAsync(new ArraySegment<byte>(Encoding.UTF8.GetBytes(msg)), WebSocketMessageType.Text, true, CancellationToken.None).Wait();
            var buf = new byte[1 << 20];
            var deadline = DateTime.UtcNow.AddSeconds(20);
            while (DateTime.UtcNow < deadline)
            {
                var sb = new StringBuilder();
                WebSocketReceiveResult r;
                do
                {
                    r = ws.ReceiveAsync(new ArraySegment<byte>(buf), CancellationToken.None).Result;
                    sb.Append(Encoding.UTF8.GetString(buf, 0, r.Count));
                } while (!r.EndOfMessage);
                string reply = sb.ToString();
                if (reply.Contains("\"id\":" + id + ",") || reply.Contains("\"id\":" + id + "}")) return reply;
            }
            return "timeout";
        }
    }
}
