{{- define "rehydration-kernel.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "rehydration-kernel.fullname" -}}
{{- if .Values.fullnameOverride -}}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- $name := default .Chart.Name .Values.nameOverride -}}
{{- if contains $name .Release.Name -}}
{{- .Release.Name | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{- define "rehydration-kernel.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "rehydration-kernel.labels" -}}
helm.sh/chart: {{ include "rehydration-kernel.chart" . }}
app.kubernetes.io/name: {{ include "rehydration-kernel.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end -}}

{{- define "rehydration-kernel.selectorLabels" -}}
app.kubernetes.io/name: {{ include "rehydration-kernel.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{- define "rehydration-kernel.serviceAccountName" -}}
{{- if .Values.serviceAccount.create -}}
{{- default (include "rehydration-kernel.fullname" .) .Values.serviceAccount.name -}}
{{- else -}}
{{- default "default" .Values.serviceAccount.name -}}
{{- end -}}
{{- end -}}

{{- define "rehydration-kernel.validateValues" -}}
{{- $tag := default "" .Values.image.tag -}}
{{- $digest := default "" .Values.image.digest -}}
{{- $allowMutableTags := default false .Values.development.allowMutableImageTags -}}
{{- $allowInlineConnections := default false .Values.development.allowInlineConnections -}}
{{- $grpcTlsMode := default "disabled" .Values.tls.mode -}}
{{- $natsTlsMode := default "disabled" .Values.natsTls.mode -}}
{{- $natsTlsSecret := default "" .Values.natsTls.existingSecret -}}
{{- $natsTlsMountPath := default "" .Values.natsTls.mountPath -}}
{{- $natsTlsCaKey := default "" .Values.natsTls.keys.ca -}}
{{- $natsTlsCertKey := default "" .Values.natsTls.keys.cert -}}
{{- $natsTlsKeyKey := default "" .Values.natsTls.keys.key -}}
{{- $ingressEnabled := default false .Values.ingress.enabled -}}
{{- $ingressHosts := default (list) .Values.ingress.hosts -}}
{{- $neo4jTlsEnabled := default false .Values.neo4jTls.enabled -}}
{{- $neo4jTlsSecret := default "" .Values.neo4jTls.existingSecret -}}
{{- $neo4jTlsMountPath := default "" .Values.neo4jTls.mountPath -}}
{{- $neo4jTlsCaKey := default "" .Values.neo4jTls.keys.ca -}}
{{- $valkeyTlsEnabled := default false .Values.valkeyTls.enabled -}}
{{- $valkeyTlsSecret := default "" .Values.valkeyTls.existingSecret -}}
{{- $valkeyTlsMountPath := default "" .Values.valkeyTls.mountPath -}}
{{- $valkeyTlsCaKey := default "" .Values.valkeyTls.keys.ca -}}
{{- $valkeyTlsCertKey := default "" .Values.valkeyTls.keys.cert -}}
{{- $valkeyTlsKeyKey := default "" .Values.valkeyTls.keys.key -}}
{{- $neo4jEnabled := default false .Values.neo4j.enabled -}}
{{- if and (eq $tag "") (eq $digest "") -}}
{{- fail "set image.tag or image.digest; the chart no longer defaults to latest" -}}
{{- end -}}
{{- if and (eq $digest "") (eq $tag "latest") (not $allowMutableTags) -}}
{{- fail "image.tag=latest requires development.allowMutableImageTags=true" -}}
{{- end -}}
{{- if not (has $grpcTlsMode (list "disabled" "server" "mutual")) -}}
{{- fail "tls.mode must be one of disabled, server, mutual" -}}
{{- end -}}
{{- if not (has $natsTlsMode (list "disabled" "server" "mutual")) -}}
{{- fail "natsTls.mode must be one of disabled, server, mutual" -}}
{{- end -}}
{{- if and (eq (default "" .Values.secrets.existingSecret) "") (not $allowInlineConnections) -}}
{{- fail "set secrets.existingSecret for connection URIs or explicitly enable development.allowInlineConnections=true" -}}
{{- end -}}
{{- if and $ingressEnabled (eq (len $ingressHosts) 0) -}}
{{- fail "ingress.hosts must contain at least one host when ingress.enabled=true" -}}
{{- end -}}
{{- if and $neo4jTlsEnabled (eq $neo4jTlsSecret "") (ne $neo4jTlsCaKey "") -}}
{{- fail "neo4jTls.existingSecret is required when neo4jTls.keys.ca is configured" -}}
{{- end -}}
{{- if and (ne $neo4jTlsSecret "") (eq $neo4jTlsMountPath "") -}}
{{- fail "neo4jTls.mountPath is required when neo4jTls.existingSecret is set" -}}
{{- end -}}
{{- if and $neo4jTlsEnabled (eq $neo4jTlsCaKey "") -}}
{{- fail "neo4jTls.keys.ca is required when neo4jTls.enabled=true" -}}
{{- end -}}
{{- if ne $grpcTlsMode "disabled" -}}
{{- if eq (default "" .Values.tls.existingSecret) "" -}}
{{- fail "tls.existingSecret is required when tls.mode is server or mutual" -}}
{{- end -}}
{{- if eq (default "" .Values.tls.mountPath) "" -}}
{{- fail "tls.mountPath is required when tls.mode is server or mutual" -}}
{{- end -}}
{{- if eq (default "" .Values.tls.keys.cert) "" -}}
{{- fail "tls.keys.cert is required when tls.mode is server or mutual" -}}
{{- end -}}
{{- if eq (default "" .Values.tls.keys.key) "" -}}
{{- fail "tls.keys.key is required when tls.mode is server or mutual" -}}
{{- end -}}
{{- if and (eq $grpcTlsMode "mutual") (eq (default "" .Values.tls.keys.clientCa) "") -}}
{{- fail "tls.keys.clientCa is required when tls.mode=mutual" -}}
{{- end -}}
{{- end -}}
{{- if and (ne $natsTlsMode "disabled") (eq $natsTlsSecret "") (or (ne $natsTlsCaKey "") (ne $natsTlsCertKey "") (ne $natsTlsKeyKey "")) -}}
{{- fail "natsTls.existingSecret is required when natsTls.keys.* are configured" -}}
{{- end -}}
{{- if and (ne $natsTlsSecret "") (eq $natsTlsMountPath "") -}}
{{- fail "natsTls.mountPath is required when natsTls.existingSecret is set" -}}
{{- end -}}
{{- if and (eq $natsTlsMode "mutual") (eq $natsTlsSecret "") -}}
{{- fail "natsTls.existingSecret is required when natsTls.mode=mutual" -}}
{{- end -}}
{{- if and (eq $natsTlsMode "mutual") (or (eq $natsTlsCertKey "") (eq $natsTlsKeyKey "")) -}}
{{- fail "natsTls.keys.cert and natsTls.keys.key are required when natsTls.mode=mutual" -}}
{{- end -}}
{{- if and (or (eq $natsTlsCertKey "") (eq $natsTlsKeyKey "")) (not (and (eq $natsTlsCertKey "") (eq $natsTlsKeyKey ""))) -}}
{{- fail "natsTls.keys.cert and natsTls.keys.key must be configured together" -}}
{{- end -}}
{{- if and $valkeyTlsEnabled (eq $valkeyTlsSecret "") (or (ne $valkeyTlsCaKey "") (ne $valkeyTlsCertKey "") (ne $valkeyTlsKeyKey "")) -}}
{{- fail "valkeyTls.existingSecret is required when valkeyTls.keys.* are configured" -}}
{{- end -}}
{{- if and (ne $valkeyTlsSecret "") (eq $valkeyTlsMountPath "") -}}
{{- fail "valkeyTls.mountPath is required when valkeyTls.existingSecret is set" -}}
{{- end -}}
{{- if and (or (eq $valkeyTlsCertKey "") (eq $valkeyTlsKeyKey "")) (not (and (eq $valkeyTlsCertKey "") (eq $valkeyTlsKeyKey ""))) -}}
{{- fail "valkeyTls.keys.cert and valkeyTls.keys.key must be configured together" -}}
{{- end -}}
{{- if $allowInlineConnections -}}
{{- if and (not $neo4jEnabled) (eq (default "" .Values.connections.graphUri) "") -}}
{{- fail "connections.graphUri is required when development.allowInlineConnections=true" -}}
{{- end -}}
{{- if eq (default "" .Values.connections.detailUri) "" -}}
{{- fail "connections.detailUri is required when development.allowInlineConnections=true" -}}
{{- end -}}
{{- if eq (default "" .Values.connections.snapshotUri) "" -}}
{{- fail "connections.snapshotUri is required when development.allowInlineConnections=true" -}}
{{- end -}}
{{- if eq (default "" .Values.connections.runtimeStateUri) "" -}}
{{- fail "connections.runtimeStateUri is required when development.allowInlineConnections=true" -}}
{{- end -}}
{{- if eq (default "" .Values.connections.natsUrl) "" -}}
{{- fail "connections.natsUrl is required when development.allowInlineConnections=true" -}}
{{- end -}}
{{- if $neo4jTlsEnabled -}}
{{- if not (or (hasPrefix "bolt+s://" .Values.connections.graphUri) (hasPrefix "bolt+ssc://" .Values.connections.graphUri) (hasPrefix "neo4j+s://" .Values.connections.graphUri) (hasPrefix "neo4j+ssc://" .Values.connections.graphUri)) -}}
{{- fail "neo4jTls.enabled requires connections.graphUri to use bolt+s://, bolt+ssc://, neo4j+s://, or neo4j+ssc:// when development.allowInlineConnections=true" -}}
{{- end -}}
{{- end -}}
{{- if $valkeyTlsEnabled -}}
{{- range $connection := list .Values.connections.detailUri .Values.connections.snapshotUri .Values.connections.runtimeStateUri -}}
{{- if not (or (hasPrefix "redis://" $connection) (hasPrefix "valkey://" $connection) (hasPrefix "rediss://" $connection) (hasPrefix "valkeys://" $connection)) -}}
{{- fail "valkeyTls.enabled requires Valkey connection URIs to use redis://, valkey://, rediss://, or valkeys:// when development.allowInlineConnections=true" -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{- define "rehydration-kernel.neo4j.fullname" -}}
{{- printf "%s-neo4j" (include "rehydration-kernel.fullname" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "rehydration-kernel.neo4j.username" -}}
{{- $authParts := splitList "/" (default "neo4j/underpassai" .Values.neo4j.auth) -}}
{{- if gt (len $authParts) 0 -}}
{{- index $authParts 0 -}}
{{- else -}}
neo4j
{{- end -}}
{{- end -}}

{{- define "rehydration-kernel.neo4j.password" -}}
{{- $authParts := splitList "/" (default "neo4j/underpassai" .Values.neo4j.auth) -}}
{{- if gt (len $authParts) 1 -}}
{{- index $authParts 1 -}}
{{- else -}}
underpassai
{{- end -}}
{{- end -}}

{{- define "rehydration-kernel.neo4j.uri" -}}
{{- printf "neo4j://%s:%s@%s:7687" (include "rehydration-kernel.neo4j.username" .) (include "rehydration-kernel.neo4j.password" .) (include "rehydration-kernel.neo4j.fullname" .) -}}
{{- end -}}

{{- define "rehydration-kernel.inlineValkeyUri" -}}
{{- $uri := .uri -}}
{{- $tls := .tls -}}
{{- if not $tls.enabled -}}
{{- $uri -}}
{{- else -}}
{{- $secureUri := $uri -}}
{{- if hasPrefix "redis://" $secureUri -}}
{{- $secureUri = printf "rediss://%s" (trimPrefix "redis://" $secureUri) -}}
{{- else if hasPrefix "valkey://" $secureUri -}}
{{- $secureUri = printf "valkeys://%s" (trimPrefix "valkey://" $secureUri) -}}
{{- end -}}
{{- $params := list -}}
{{- if and (ne (default "" $tls.existingSecret) "") (ne (default "" $tls.keys.ca) "") -}}
{{- $params = append $params (printf "tls_ca_path=%s/%s" $tls.mountPath $tls.keys.ca) -}}
{{- end -}}
{{- if and (ne (default "" $tls.existingSecret) "") (ne (default "" $tls.keys.cert) "") (ne (default "" $tls.keys.key) "") -}}
{{- $params = append $params (printf "tls_cert_path=%s/%s" $tls.mountPath $tls.keys.cert) -}}
{{- $params = append $params (printf "tls_key_path=%s/%s" $tls.mountPath $tls.keys.key) -}}
{{- end -}}
{{- if gt (len $params) 0 -}}
{{- if contains "?" $secureUri -}}
{{- printf "%s&%s" $secureUri (join "&" $params) -}}
{{- else -}}
{{- printf "%s?%s" $secureUri (join "&" $params) -}}
{{- end -}}
{{- else -}}
{{- $secureUri -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{- define "rehydration-kernel.inlineGraphUri" -}}
{{- $uri := .uri -}}
{{- $tls := .tls -}}
{{- if not $tls.enabled -}}
{{- $uri -}}
{{- else -}}
{{- $params := list -}}
{{- if and (ne (default "" $tls.existingSecret) "") (ne (default "" $tls.keys.ca) "") -}}
{{- $params = append $params (printf "tls_ca_path=%s/%s" $tls.mountPath $tls.keys.ca) -}}
{{- end -}}
{{- if gt (len $params) 0 -}}
{{- if contains "?" $uri -}}
{{- printf "%s&%s" $uri (join "&" $params) -}}
{{- else -}}
{{- printf "%s?%s" $uri (join "&" $params) -}}
{{- end -}}
{{- else -}}
{{- $uri -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{- define "rehydration-kernel.image" -}}
{{- $repository := .Values.image.repository -}}
{{- $tag := default "" .Values.image.tag -}}
{{- $digest := default "" .Values.image.digest -}}
{{- if ne $digest "" -}}
{{- printf "%s@%s" $repository $digest -}}
{{- else -}}
{{- printf "%s:%s" $repository $tag -}}
{{- end -}}
{{- end -}}
