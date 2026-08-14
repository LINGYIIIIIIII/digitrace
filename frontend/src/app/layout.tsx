import type { Metadata } from "next";
import "@fontsource-variable/manrope/index.css";
import "@fontsource-variable/geist-mono/index.css";
import "./globals.css";
import SystemThemeSync from "./components/SystemThemeSync";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { BRAND } from "./lib/brand";
import { AppI18nProvider } from "./lib/i18n";
import { getThemeBootstrapScript } from "./lib/theme-bootstrap";

export const metadata: Metadata = {
  title: BRAND.name,
  description: BRAND.description,
};

/** 键盘焦点守卫：仅在真实键盘导航时开启焦点圈，鼠标/触摸/窗口重新聚焦不显示。 */
const KEYBOARD_FOCUS_GUARD = `(function(){var kb=false;function off(){if(kb){kb=false;document.body.classList.remove('kbd-focus')}}window.addEventListener('keydown',function(e){if(e.key==='Tab'||e.key.indexOf('Arrow')===0){if(!kb){kb=true;document.body.classList.add('kbd-focus')}}},true);window.addEventListener('mousedown',off,true);window.addEventListener('pointerdown',off,true);window.addEventListener('touchstart',off,true);})();`;

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  const themeBootstrapScript = getThemeBootstrapScript();

  return (
    <html lang="zh-CN" suppressHydrationWarning>
      <head>
        <script
          id="digitrace-theme-bootstrap"
          dangerouslySetInnerHTML={{ __html: themeBootstrapScript }}
        />
        <script
          id="digitrace-keyboard-focus-guard"
          dangerouslySetInnerHTML={{ __html: KEYBOARD_FOCUS_GUARD }}
        />
      </head>
      <body>
        <AppI18nProvider>
          <SystemThemeSync />
          <TooltipProvider delayDuration={180}>
            {children}
            <Toaster richColors closeButton position="top-right" />
          </TooltipProvider>
        </AppI18nProvider>
      </body>
    </html>
  );
}
