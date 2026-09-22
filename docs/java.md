# Java

Detection checks JAVA_HOME, absolute PATH entries, common Windows vendors, standard macOS/Linux directories, and NodeClient/java. Validation invokes java -version with an argument array, a ten-second timeout, and hidden Windows console. javaw.exe selections are probed using sibling java.exe.

Required major version comes from javaVersion.majorVersion; legacy metadata defaults to Java 8. Automatic mode selects an exact major match. If none is available on Windows x64, the Mojang java-runtime product manifest determines the component and every raw file hash/size. Files are installed under java/<component>, then the executable is validated.

The managed installer rejects archive links and unknown entries rather than trusting arbitrary executables or URLs. It never replaces a system Java installation. Windows x64 managed installation is implemented; other platforms require a compatible preinstalled runtime until their packaging/layouts are validated.

Custom Java is configured per instance through a native file picker. Memory defaults to approximately one third of physical RAM, capped at 4 GB. The instance editor limits its slider to 75% of reported RAM.
