# Gradle lint precision

Sample: seed 20261204, up to 50 random findings per rule from `sudsy lint` with presets gradle/modernize and gradle/codenarc over the 6,481 corpus files. Each finding was judged TRUE or FALSE by a reader that did not write the rule. The three rules unused-variable (50/50), no-legacy-archive-property (3/3) and no-main-class-name (15/15) are included as already judged.

File paths below are relative to `tests/conformance/corpus/gradle/`. Findings column: total findings for the rule in the sample run.

| rule | findings | judged | true | precision % | false-positive causes |
|---|---|---|---|---|---|
| no-apply-plugin | 3038 | 50 | 50 | 100.0 | none |
| no-build-dir | 338 | 50 | 47 | 94.0 | local `buildDir` variable shadows project property (3) |
| no-build-tools-version | 99 | 50 | 50 | 100.0 | none |
| no-compile-sdk-version | 118 | 50 | 50 | 100.0 | none |
| no-floating-version | 121 | 50 | 50 | 100.0 | none |
| no-gstring-map-key | 2 | 2 | 2 | 100.0 | none |
| no-java-convention-compatibility | 34 | 34 | 34 | 100.0 | none |
| no-jcenter | 23 | 23 | 23 | 100.0 | none |
| no-legacy-archive-property | 3 | 3 | 3 | 100.0 | none (already judged) |
| no-lint-options | 59 | 50 | 50 | 100.0 | none |
| no-main-class-name | 15 | 15 | 15 | 100.0 | none (already judged) |
| no-removed-configuration | 71 | 50 sampled before the `constraints {}` fix; 4 false | 46 | 92.0 (at most 5 false among the 71 findings now: 93.0) | file defines its own compile/runtime configuration (4); the 8 java-platform `constraints {}` hits are fixed and no longer reported |
| no-tasks-create | 31 | 31 | 31 | 100.0 | none |
| no-upload-archives | 1 | 1 | 1 | 100.0 | none |
| unreachable-code | 1 | 1 | 1 | 100.0 | none |
| unused-import | 51 | 50 | 50 | 100.0 | none |
| unused-variable | 51 | 50 | 50 | 100.0 | none (already judged) |
| **all rules** | 4073 | 560 | 545 | 97.3 | |

## FALSE findings

no-build-dir:
- hibernate__hibernate-orm/hibernate-tck-data-runner/hibernate-tck-data-runner.gradle:145 - local def buildDir shadows project property
- spring-projects__spring-framework/settings.gradle:44 - local File buildDir, not project property
- spring-projects__spring-framework/settings.gradle:47 - local File buildDir, not project property

no-removed-configuration:
- gradle__gradle/platforms/software/dependency-management/src/integTest/resources/org/gradle/integtests/resolve/ArtifactDependenciesIntegrationTest/canHaveConfigurationHierarchy/build.gradle:12 - file defines own compile configuration
- gradle__gradle/platforms/software/dependency-management/src/integTest/resources/org/gradle/integtests/resolve/ArtifactDependenciesIntegrationTest/canHaveConfigurationHierarchy/build.gradle:16 - file defines own runtime configuration
- gradle__gradle/platforms/software/dependency-management/src/integTest/resources/org/gradle/integtests/resolve/ArtifactDependenciesIntegrationTest/canHaveCycleInDependencyGraph/build.gradle:11 - file defines own compile configuration
- gradle__gradle/platforms/software/dependency-management/src/integTest/resources/org/gradle/integtests/resolve/ArtifactDependenciesIntegrationTest/canHaveConfigurationHierarchy/build.gradle:15 - file defines own runtime configuration

## Result
- Zero-config rules (no-removed-configuration, no-jcenter, no-lint-options, no-legacy-archive-property, no-main-class-name, no-upload-archives, no-gstring-map-key, unreachable-code): 141 true of 145 judged, 97.2%; every rule at least 92%.
- Presets gradle/modernize and gradle/codenarc (off by default): every rule at least 94%.
- No finding in the corpus for `no-empty-block` (fixture only; CodeNarc source in the rule docs).
