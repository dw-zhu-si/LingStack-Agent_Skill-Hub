import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const common = {
  privacy_url: "https://pm.jcm99.com/apple/lingstack/privacy.html",
  support_url: "https://github.com/dw-zhu-si/LingStack-Agent_Skill-Hub/blob/main/SUPPORT.md",
  marketing_url: ""
};

const locales = {
  "zh-Hans": {
    name: "灵栈：Agent 与 Skill 中枢",
    subtitle: "统一管理本地 AI 能力资产",
    promotional_text: "把分散在多个开发工具中的 Agent 与 Skill 统一带回一张地图。灵栈帮你发现资产、识别版本冲突、验证结构并获得优化建议；路径变更与版本选择始终由你手动触发，让每一次关键操作都有依据、可确认、可回看。",
    keywords: "Agent,Skill,开发工具,本地管理,自动验真,提示词,项目地图,版本管理,资产",
    description: "灵栈（LingStack）是一款本地优先的 Agent 与 Skill 统一管理器。它帮助你发现分散在不同开发工具与项目中的能力资产，并用来源、哈希、版本和项目关系建立清晰地图。\n\n灵栈提供只读结构审计、优化建议、版本冲突处置、可追溯状态和安全导出。\n\n路径切换、版本选择、许可确认与晋升不会自动执行，始终需要用户手动触发。公开发行版不内置任何用户 Agent 或 Skill。"
  },
  "zh-Hant": {
    name: "靈棧：Agent 與 Skill 中樞",
    subtitle: "統一管理本機 AI 能力資產",
    promotional_text: "探索、驗真並優化本機 Agent 與 Skill；統一理解專案關係和工具路徑，所有關鍵變更皆由你手動觸發。",
    keywords: "Agent,Skill,開發工具,本機管理,驗真,提示詞,專案地圖,版本管理,資產",
    description: "靈棧（LingStack）是一款本機優先的 Agent 與 Skill 統一管理器。它協助你探索分散於不同開發工具與專案中的能力資產，並以來源、雜湊、版本和專案關係建立清晰地圖。\n\n靈棧提供唯讀結構稽核、優化建議、版本衝突處置、可追溯狀態和安全匯出。\n\n路徑切換、版本選擇、授權確認與晉升不會自動執行，始終需要使用者手動觸發。公開發行版不內建任何使用者 Agent 或 Skill。"
  },
  "en-US": {
    name: "LingStack: Agent & Skill Hub",
    subtitle: "Local AI assets, one clear hub",
    promotional_text: "Discover, verify, and improve local Agents and Skills. Understand project relationships and tool paths with every critical change under your control.",
    keywords: "agent,skill,developer tools,local,verification,prompts,project map,versions,assets",
    description: "LingStack is a local-first hub for Agents and Skills. It discovers capability definitions across supported developer tools and projects, then organizes them by source, content hash, version, and project relationship.\n\nLingStack provides read-only structural audits, optimization suggestions, version-conflict triage, traceable states, and clean exports.\n\nTool-path changes, version selection, license confirmation, and promotion are never applied automatically. They always require an explicit user action. The public release bundles no personal Agents or Skills."
  },
  ja: {
    name: "LingStack: Agent & Skill Hub",
    subtitle: "ローカルAI資産を一元管理",
    promotional_text: "ローカルのAgentとSkillを発見・検証・改善。プロジェクト関係とツールパスを整理し、重要な変更は必ず手動で実行します。",
    keywords: "Agent,Skill,開発ツール,ローカル,検証,プロンプト,プロジェクト,バージョン,資産",
    description: "LingStackは、AgentとSkillのためのローカルファーストな統合管理ハブです。複数の開発ツールやプロジェクトに分散した定義を検出し、出典、ハッシュ、バージョン、プロジェクト関係で整理します。\n\n読み取り専用監査、最適化提案、バージョン競合の整理、追跡可能な状態、安全なエクスポートに対応します。\n\nツールパス変更、バージョン選択、ライセンス確認、昇格は自動実行されず、常にユーザーの明示操作が必要です。公開版には個人のAgentやSkillを同梱しません。"
  },
  ko: {
    name: "LingStack: Agent & Skill Hub",
    subtitle: "로컬 AI 자산 통합 관리",
    promotional_text: "로컬 Agent와 Skill을 탐색·검증·개선하세요. 프로젝트 관계와 도구 경로를 정리하고 중요한 변경은 직접 실행합니다.",
    keywords: "Agent,Skill,개발도구,로컬,검증,프롬프트,프로젝트,버전,자산",
    description: "LingStack은 Agent와 Skill을 위한 로컬 우선 통합 관리 허브입니다. 여러 개발 도구와 프로젝트에 흩어진 정의를 찾아 출처, 해시, 버전, 프로젝트 관계로 정리합니다.\n\n읽기 전용 구조 감사, 최적화 제안, 버전 충돌 분류, 추적 가능한 상태, 안전한 내보내기를 제공합니다.\n\n도구 경로 변경, 버전 선택, 라이선스 확인, 승격은 자동 실행되지 않으며 항상 사용자의 명시적 작업이 필요합니다. 공개 배포판에는 개인 Agent나 Skill이 포함되지 않습니다."
  },
  "fr-FR": {
    name: "LingStack: Agent & Skill Hub",
    subtitle: "Vos actifs IA locaux unifiés",
    promotional_text: "Découvrez, vérifiez et améliorez vos Agents et Skills locaux. Organisez projets et chemins d’outils avec des changements toujours manuels.",
    keywords: "agent,skill,développement,local,vérification,prompts,projets,versions,actifs",
    description: "LingStack est un centre local-first pour les Agents et Skills. Il découvre les définitions réparties entre vos outils de développement et vos projets, puis les organise par source, empreinte, version et relation de projet.\n\nLingStack propose des audits structurels en lecture seule, des suggestions d’optimisation, la gestion des conflits de version, des états traçables et des exports propres.\n\nLes changements de chemins, choix de version, confirmations de licence et promotions ne sont jamais automatiques : ils nécessitent toujours une action explicite. La version publique n’intègre aucun Agent ou Skill personnel."
  },
  "de-DE": {
    name: "LingStack: Agent & Skill Hub",
    subtitle: "Lokale KI-Assets im Überblick",
    promotional_text: "Lokale Agents und Skills finden, prüfen und verbessern. Projektbeziehungen und Tool-Pfade bleiben mit manuellen Änderungen unter Ihrer Kontrolle.",
    keywords: "Agent,Skill,Entwicklung,lokal,Prüfung,Prompts,Projekte,Versionen,Assets",
    description: "LingStack ist eine Local-first-Zentrale für Agents und Skills. Definitionen aus Entwicklungstools und Projekten werden erkannt und nach Quelle, Hash, Version und Projektbeziehung geordnet.\n\nHinzu kommen schreibgeschützte Strukturprüfungen, Optimierungsvorschläge, Versionskonflikte, nachvollziehbare Zustände und saubere Exporte.\n\nTool-Pfade, Versionsauswahl, Lizenzbestätigung und Freigabe werden niemals automatisch geändert. Jede Aktion muss ausdrücklich ausgelöst werden. Die öffentliche Version enthält keine persönlichen Agents oder Skills."
  },
  "es-ES": {
    name: "LingStack: Agent & Skill Hub",
    subtitle: "Tus activos de IA local",
    promotional_text: "Descubre, verifica y mejora Agents y Skills locales. Organiza proyectos y rutas de herramientas con cambios críticos siempre manuales.",
    keywords: "agent,skill,desarrollo,local,verificación,prompts,proyectos,versiones,activos",
    description: "LingStack es un centro local-first para Agents y Skills. Descubre definiciones repartidas entre herramientas de desarrollo y proyectos, y las organiza por origen, hash, versión y relación con el proyecto.\n\nTambién incluye auditorías estructurales de solo lectura, sugerencias de optimización, gestión de conflictos de versión, estados trazables y exportaciones limpias.\n\nLos cambios de rutas, la selección de versión, la confirmación de licencias y la promoción nunca se ejecutan automáticamente; siempre requieren una acción explícita. La versión pública no incluye Agents ni Skills personales."
  },
  "pt-BR": {
    name: "LingStack: Agent & Skill Hub",
    subtitle: "Ativos locais de IA",
    promotional_text: "Descubra, valide e melhore Agents e Skills locais. Organize projetos e caminhos de ferramentas com mudanças sempre manuais.",
    keywords: "agent,skill,desenvolvimento,local,validação,prompts,projetos,versões,ativos",
    description: "LingStack é um hub local-first para Agents e Skills. Ele encontra definições distribuídas entre ferramentas de desenvolvimento e projetos, organizando tudo por origem, hash, versão e relação com o projeto.\n\nO app também oferece auditorias estruturais somente leitura, sugestões de otimização, tratamento de conflitos de versão, estados rastreáveis e exportações limpas.\n\nMudanças de caminhos, seleção de versão, confirmação de licença e promoção nunca são automáticas: sempre exigem uma ação explícita. A versão pública não inclui Agents ou Skills pessoais."
  },
  ru: {
    name: "LingStack: Agent & Skill Hub",
    subtitle: "Локальные ИИ-ресурсы",
    promotional_text: "Находите, проверяйте и улучшайте локальные Agents и Skills. Связи проектов и пути инструментов меняются только вручную.",
    keywords: "Agent,Skill,разработка,локально,проверка,промпты,проекты,версии,ресурсы",
    description: "LingStack — локальный центр управления Agents и Skills. Он находит определения в инструментах разработки и проектах, а затем группирует их по источнику, хешу, версии и связи с проектом.\n\nТакже доступны структурный аудит только для чтения, рекомендации по оптимизации, разбор конфликтов версий, отслеживаемые состояния и безопасный экспорт.\n\nПути инструментов, выбор версии, подтверждение лицензии и повышение статуса никогда не меняются автоматически. Для них всегда требуется явное действие пользователя. Публичная версия не содержит личных Agents или Skills."
  },
  "ar-SA": {
    name: "LingStack: Agent & Skill Hub",
    subtitle: "إدارة موحدة لأصول الذكاء",
    promotional_text: "اكتشف Agents وSkills المحلية وتحقق منها وحسّنها. نظّم علاقات المشاريع ومسارات الأدوات مع إبقاء كل تغيير مهم بقرار يدوي.",
    keywords: "Agent,Skill,تطوير,محلي,تحقق,مطالبات,مشاريع,إصدارات,أصول",
    description: "LingStack هو مركز محلي أولاً لإدارة Agents وSkills. يكتشف التعريفات الموزعة بين أدوات التطوير والمشاريع، ثم ينظمها حسب المصدر والبصمة والإصدار وعلاقة المشروع.\n\nكما يوفر تدقيقاً هيكلياً للقراءة فقط واقتراحات تحسين ومعالجة تعارض الإصدارات وحالات قابلة للتتبع وتصديراً آمناً.\n\nلا تُنفذ تغييرات المسارات أو اختيار الإصدار أو تأكيد الترخيص أو الترقية تلقائياً، بل تتطلب دائماً إجراءً صريحاً من المستخدم. لا تتضمن النسخة العامة أي Agents أو Skills شخصية."
  }
};

for (const [locale, metadata] of Object.entries(locales)) {
  const directory = join(root, "metadata", locale);
  await mkdir(directory, { recursive: true });
  for (const [key, value] of Object.entries({ ...metadata, ...common })) {
    await writeFile(join(directory, `${key}.txt`), `${value.trim()}\n`, "utf8");
  }
}

console.log(`Generated ${Object.keys(locales).length} App Store localizations.`);
