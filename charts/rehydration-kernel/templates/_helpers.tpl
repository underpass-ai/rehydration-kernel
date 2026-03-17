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
{{- if and (eq $tag "") (eq $digest "") -}}
{{- fail "set image.tag or image.digest; the chart no longer defaults to latest" -}}
{{- end -}}
{{- if and (eq $digest "") (eq $tag "latest") (not $allowMutableTags) -}}
{{- fail "image.tag=latest requires development.allowMutableImageTags=true" -}}
{{- end -}}
{{- if not (has $grpcTlsMode (list "disabled" "server" "mutual")) -}}
{{- fail "tls.mode must be one of disabled, server, mutual" -}}
{{- end -}}
{{- if and (eq (default "" .Values.secrets.existingSecret) "") (not $allowInlineConnections) -}}
{{- fail "set secrets.existingSecret for connection URIs or explicitly enable development.allowInlineConnections=true" -}}
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
{{- if $allowInlineConnections -}}
{{- if eq (default "" .Values.connections.graphUri) "" -}}
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
