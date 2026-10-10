package com.github.fprime_community.fpp_tools

import com.github.fprime_community.fpp_tools.diagram.DiagramElement
import com.github.fprime_community.fpp_tools.diagram.DiagramElementsParams
import com.github.fprime_community.fpp_tools.diagram.DiagramParams
import com.github.fprime_community.fpp_tools.settings.FppSettingsConfigurable
import com.github.fprime_community.fpp_tools.util.LspCli
import com.google.gson.JsonElement
import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.icons.AllIcons
import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.editor.colors.TextAttributesKey
import com.intellij.openapi.progress.currentThreadCoroutineScope
import com.intellij.openapi.project.DumbAware
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.*
import com.intellij.platform.lsp.api.customization.LspCustomization
import com.intellij.platform.lsp.api.customization.LspSemanticTokensSupport
import com.intellij.platform.lsp.api.lsWidget.LspServerWidgetItem
import com.intellij.util.containers.addIfNotNull
import kotlinx.coroutines.launch
import org.eclipse.lsp4j.jsonrpc.services.JsonRequest
import org.eclipse.lsp4j.services.LanguageServer
import java.util.concurrent.CompletableFuture

private class ReloadWorkspaceAction(
    private val lspServer: LspServer,
) : AnAction(
    "Reload Workspace",
    "Reload the FPP workspace by re-running project discovery (.fpp-lsp)",
    AllIcons.Actions.Refresh
), DumbAware {
    override fun actionPerformed(e: AnActionEvent): Unit =
        run { reloadWorkspace(lspServer) }
}

open class FppLspProjectWidgetItem(lspServer: LspServer, currentFile: VirtualFile?) : LspServerWidgetItem(
    lspServer, currentFile,
    settingsPageClass = FppSettingsConfigurable::class.java
) {
    override fun createWidgetInlineActions(): List<AnAction> {
        val actions = super.createWidgetInlineActions().toMutableList()
        actions.addIfNotNull(ReloadWorkspaceAction(lspServer))

        return actions
    }
}

internal class FppLspServerSupportProvider : LspServerSupportProvider {
    override fun fileOpened(
        project: Project,
        file: VirtualFile,
        serverStarter: LspServerSupportProvider.LspServerStarter
    ) {
        if (file.extension == "fpp" || file.extension == "fppi") {
            serverStarter.ensureServerStarted(FppLspServerDescriptor(project))
        }
    }

    override fun createLspServerWidgetItem(lspServer: LspServer, currentFile: VirtualFile?): LspServerWidgetItem =
        FppLspProjectWidgetItem(lspServer, currentFile)
}

private class FppLspServerDescriptor(project: Project) : ProjectWideLspServerDescriptor(project, "fpp") {
    override fun isSupportedFile(file: VirtualFile) = file.extension == "fpp" || file.extension == "fppi"
    override fun createCommandLine(): GeneralCommandLine {
        val lspConfiguration = project.getLspConfiguration()
        if (lspConfiguration is LspConfiguration.Disabled) {
            throw IllegalStateException(lspConfiguration.message)
        }
        if (lspConfiguration !is LspConfiguration.Enabled) {
            throw IllegalStateException("Tried to created a FPP LSP with disabled configuration")
        }
        return LspCli(project, lspConfiguration).createLspCli()
    }

    override val lsp4jServerClass = FppLsp4jServer::class.java

    override val lspCustomization: LspCustomization
        get() = object : LspCustomization() {
            override val semanticTokensCustomizer = object : LspSemanticTokensSupport() {
                override fun getTextAttributesKey(tokenType: String, modifiers: List<String>): TextAttributesKey? {
                    return when (tokenType) {
                        SemanticTokenTypes.Module -> FppColors.MODULE
                        SemanticTokenTypes.Topology -> FppColors.TOPOLOGY
                        SemanticTokenTypes.Component -> FppColors.COMPONENT
                        SemanticTokenTypes.Interface -> FppColors.INTERFACE
                        SemanticTokenTypes.ComponentInstance -> FppColors.COMPONENT_INSTANCE
                        SemanticTokenTypes.Constant -> FppColors.CONSTANT
                        SemanticTokenTypes.EnumConstant -> FppColors.ENUM_CONSTANT
                        SemanticTokenTypes.StructMember -> FppColors.STRUCT_MEMBER
                        SemanticTokenTypes.GraphGroup -> FppColors.GRAPH_GROUP
                        SemanticTokenTypes.PortInstance -> FppColors.PORT_INSTANCE
                        SemanticTokenTypes.Port -> FppColors.PORT
                        SemanticTokenTypes.AbstractType -> FppColors.ABSTRACT_TYPE
                        SemanticTokenTypes.AliasType -> FppColors.ALIAS_TYPE
                        SemanticTokenTypes.ArrayType -> FppColors.ARRAY_TYPE
                        SemanticTokenTypes.EnumType -> FppColors.ENUM_TYPE
                        SemanticTokenTypes.StructType -> FppColors.STRUCT_TYPE
                        SemanticTokenTypes.PrimitiveType -> FppColors.PRIMITIVE_TYPE
                        SemanticTokenTypes.FormalParameter -> FppColors.FORMAL_PARAMETER
                        SemanticTokenTypes.Command -> FppColors.COMMAND
                        SemanticTokenTypes.Event -> FppColors.EVENT
                        SemanticTokenTypes.Telemetry -> FppColors.TELEMETRY
                        SemanticTokenTypes.Parameter -> FppColors.PARAMETER
                        SemanticTokenTypes.DataProduct -> FppColors.DATA_PRODUCT

                        SemanticTokenTypes.StateMachine -> FppColors.STATE_MACHINE
                        SemanticTokenTypes.StateMachineInstance -> FppColors.STATE_MACHINE_INSTANCE
                        SemanticTokenTypes.TelemetryPacketSet -> FppColors.TELEMETRY_PACKET_SET
                        SemanticTokenTypes.TelemetryPacket -> FppColors.TELEMETRY_PACKET

                        SemanticTokenTypes.Action -> FppColors.ACTION
                        SemanticTokenTypes.Guard -> FppColors.GUARD
                        SemanticTokenTypes.Signal -> FppColors.SIGNAL
                        SemanticTokenTypes.State -> FppColors.STATE
                        SemanticTokenTypes.Choice -> FppColors.CHOICE

                        SemanticTokenTypes.Template -> FppColors.TEMPLATE
                        SemanticTokenTypes.TemplateParameter -> FppColors.TEMPLATE_PARAMETER

                        // Other
                        SemanticTokenTypes.Annotation -> FppColors.ANNOTATION
                        SemanticTokenTypes.Comment -> FppColors.COMMENT
                        SemanticTokenTypes.Number -> FppColors.NUMBER
                        SemanticTokenTypes.String -> FppColors.STRING
                        SemanticTokenTypes.Keyword -> FppColors.KEYWORD
                        else -> null
                    }?.textAttributesKey
                }
            }
        }
}

interface FppLsp4jServer : LanguageServer {
    @JsonRequest("fpp/reloadWorkspace")
    fun reloadWorkspace(params: Void?): CompletableFuture<Void>

    /**
     * For a state machine the result is a JSON string holding Mermaid `stateDiagram-v2` source.
     * For other kinds it is the sprotty `SModel` object. The caller dispatches on the requested kind.
     */
    @JsonRequest("fpp/diagram")
    fun diagram(params: DiagramParams): CompletableFuture<JsonElement>

    /** List the diagrammable elements defined in a document. */
    @JsonRequest("fpp/diagramElements")
    fun diagramElements(params: DiagramElementsParams): CompletableFuture<List<DiagramElement>>
}

/**
 * Ask the server to re-run project discovery. Project configuration lives in the
 * workspace's `.fpp-lsp` file, which the server reads and reloads; the client only
 * needs to trigger a reload.
 */
fun reloadWorkspace(lspServer: LspServer) {
    currentThreadCoroutineScope().launch {
        lspServer.sendRequest { (it as FppLsp4jServer).reloadWorkspace(null) }
    }
}

fun Project.fppLspClients(onlyIfRunning: Boolean = true): Collection<LspClient> {
    return LspClientManager.getInstance(this)
        .getClients(FppLspServerSupportProvider::class.java)
        .filter { !onlyIfRunning || it.state == LspServerState.Running }
}
