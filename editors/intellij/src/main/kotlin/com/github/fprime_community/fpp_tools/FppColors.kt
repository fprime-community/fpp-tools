package com.github.fprime_community.fpp_tools

import com.intellij.lang.annotation.HighlightSeverity
import com.intellij.openapi.editor.HighlighterColors
import com.intellij.openapi.editor.colors.TextAttributesKey
import com.intellij.openapi.options.OptionsBundle
import com.intellij.openapi.options.colors.AttributesDescriptor
import com.intellij.openapi.util.NlsContexts.AttributeDescriptor
import java.util.function.Supplier
import com.intellij.openapi.editor.DefaultLanguageHighlighterColors as Default

/**
 * See [FppColorSettingsPage]
 */
enum class FppColors(humanName: Supplier<@AttributeDescriptor String>, default: TextAttributesKey) {
    MODULE(FppBundle.messagePointer("settings.fpp.color.module"), Default.CLASS_NAME),
    TOPOLOGY(FppBundle.messagePointer("settings.fpp.color.topology"), Default.CLASS_NAME),
    COMPONENT(FppBundle.messagePointer("settings.fpp.color.component"), Default.CLASS_NAME),
    INTERFACE(FppBundle.messagePointer("settings.fpp.color.interface"), Default.INTERFACE_NAME),
    COMPONENT_INSTANCE(FppBundle.messagePointer("settings.fpp.color.component.instance"), Default.GLOBAL_VARIABLE),
    CONSTANT(FppBundle.messagePointer("settings.fpp.color.constant"), Default.CONSTANT),
    ENUM_CONSTANT(FppBundle.messagePointer("settings.fpp.color.enum.constant"), Default.CONSTANT),
    STRUCT_MEMBER(FppBundle.messagePointer("settings.fpp.color.struct.member"), Default.INSTANCE_FIELD),
    GRAPH_GROUP(FppBundle.messagePointer("settings.fpp.color.graph.group"), Default.CLASS_NAME),
    PORT_INSTANCE(FppBundle.messagePointer("settings.fpp.color.port.instance"), Default.FUNCTION_DECLARATION),
    PORT(FppBundle.messagePointer("settings.fpp.color.port"), Default.CLASS_NAME),
    ABSTRACT_TYPE(FppBundle.messagePointer("settings.fpp.color.abstract.type"), Default.CLASS_NAME),
    ALIAS_TYPE(FppBundle.messagePointer("settings.fpp.color.alias.type"), Default.CLASS_NAME),
    ARRAY_TYPE(FppBundle.messagePointer("settings.fpp.color.array.type"), Default.CLASS_NAME),
    ENUM_TYPE(FppBundle.messagePointer("settings.fpp.color.enum.type"), Default.CLASS_NAME),
    STRUCT_TYPE(FppBundle.messagePointer("settings.fpp.color.struct.type"), Default.CLASS_NAME),
    PRIMITIVE_TYPE(FppBundle.messagePointer("settings.fpp.color.primitive.type"), Default.KEYWORD),
    FORMAL_PARAMETER(FppBundle.messagePointer("settings.fpp.color.formal.parameter"), Default.INSTANCE_FIELD),
    COMMAND(FppBundle.messagePointer("settings.fpp.color.command"), Default.FUNCTION_DECLARATION),
    EVENT(FppBundle.messagePointer("settings.fpp.color.event"), Default.FUNCTION_DECLARATION),
    TELEMETRY(FppBundle.messagePointer("settings.fpp.color.telemetry"), Default.FUNCTION_DECLARATION),
    PARAMETER(FppBundle.messagePointer("settings.fpp.color.parameter"), Default.FUNCTION_DECLARATION),
    DATA_PRODUCT(FppBundle.messagePointer("settings.fpp.color.data.product"), Default.FUNCTION_DECLARATION),

    STATE_MACHINE(FppBundle.messagePointer("settings.fpp.color.state.machine"), Default.CLASS_NAME),
    STATE_MACHINE_INSTANCE(FppBundle.messagePointer("settings.fpp.color.state.machine.instance"), Default.LOCAL_VARIABLE),
    TELEMETRY_PACKET_SET(FppBundle.messagePointer("settings.fpp.color.telemetry.packet.set"), Default.CLASS_NAME),
    TELEMETRY_PACKET(FppBundle.messagePointer("settings.fpp.color.telemetry.packet"), Default.CLASS_NAME),

    ACTION(FppBundle.messagePointer("settings.fpp.color.action"), Default.FUNCTION_DECLARATION),
    GUARD(FppBundle.messagePointer("settings.fpp.color.guard"), Default.LOCAL_VARIABLE),
    SIGNAL(FppBundle.messagePointer("settings.fpp.color.signal"), Default.FUNCTION_CALL),
    STATE(FppBundle.messagePointer("settings.fpp.color.state"), Default.GLOBAL_VARIABLE),
    CHOICE(FppBundle.messagePointer("settings.fpp.color.choice"), Default.GLOBAL_VARIABLE),

    TEMPLATE(FppBundle.messagePointer("settings.fpp.color.template"), Default.CLASS_NAME),
    TEMPLATE_PARAMETER(FppBundle.messagePointer("settings.fpp.color.template.parameter"), Default.INSTANCE_FIELD),

    // Other
    ANNOTATION(FppBundle.messagePointer("settings.fpp.color.annotation"), Default.DOC_COMMENT),
    COMMENT(FppBundle.messagePointer("settings.fpp.color.comment"), Default.LINE_COMMENT),
    NUMBER(FppBundle.messagePointer("settings.fpp.color.number"), Default.NUMBER),
    STRING(FppBundle.messagePointer("settings.fpp.color.string"), Default.STRING),
    KEYWORD(FppBundle.messagePointer("settings.fpp.color.keyword"), Default.KEYWORD);

    val textAttributesKey = TextAttributesKey.createTextAttributesKey("fpp.$name", default)
    val attributesDescriptor = AttributesDescriptor(humanName, textAttributesKey)
    val testSeverity: HighlightSeverity = HighlightSeverity(name, HighlightSeverity.INFORMATION.myVal)
}
