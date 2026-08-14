import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  output: "export",
  trailingSlash: true,
  distDir: "dist",
  images: {
    unoptimized: true,
  },
  typescript: {
    // 类型检查由 CI/tsc 独立把关；此处跳过可避免构建环境子进程受限时卡在 TypeScript 阶段。
    ignoreBuildErrors: true,
  },
  experimental: {
    useTypeScriptCli: true,
    workerThreads: true,
  },
};

export default nextConfig;
